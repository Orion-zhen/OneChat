use std::{
    borrow::Cow,
    collections::BTreeMap,
    env,
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::{
    os::unix::{ffi::OsStringExt, fs::PermissionsExt},
    time::Duration,
};

#[cfg(unix)]
const SHELL_ENV_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Debug)]
pub(super) struct ExecutionEnvironment {
    path: Vec<PathBuf>,
    #[cfg(windows)]
    path_ext: Vec<OsString>,
}

impl ExecutionEnvironment {
    pub(super) fn inherited() -> Self {
        Self {
            path: env_path("PATH"),
            #[cfg(windows)]
            path_ext: path_extensions(env::var_os("PATHEXT")),
        }
    }

    pub(super) async fn discover() -> Self {
        #[cfg(unix)]
        {
            return discover_unix().await;
        }

        #[cfg(windows)]
        {
            return discover_windows();
        }

        #[allow(unreachable_code)]
        Self::inherited()
    }

    pub(super) fn resolve(
        &self,
        command: &str,
        cwd: Option<&Path>,
        overrides: &BTreeMap<String, String>,
    ) -> Option<PathBuf> {
        let command = Path::new(command);
        if command.is_absolute() {
            return executable_candidate(command, &self.extensions(overrides));
        }
        if command.components().count() > 1 {
            return cwd.and_then(|cwd| {
                executable_candidate(&cwd.join(command), &self.extensions(overrides))
            });
        }

        let path = environment_value(overrides, "PATH")
            .map(|path| env::split_paths(OsStr::new(path)).collect())
            .unwrap_or_else(|| self.path.clone());
        let extensions = self.extensions(overrides);
        path.into_iter()
            .filter(|directory| directory.is_absolute())
            .find_map(|directory| executable_candidate(&directory.join(command), &extensions))
    }

    pub(super) fn apply(
        &self,
        command: &mut tokio::process::Command,
        overrides: &BTreeMap<String, String>,
    ) {
        if environment_value(overrides, "PATH").is_none()
            && let Ok(path) = env::join_paths(&self.path)
        {
            command.env("PATH", path);
        }
        #[cfg(windows)]
        if environment_value(overrides, "PATHEXT").is_none()
            && let Ok(path_ext) = env::join_paths(
                self.path_ext
                    .iter()
                    .filter(|extension| !extension.is_empty()),
            )
        {
            command.env("PATHEXT", path_ext);
        }
        command.envs(overrides);
    }

    #[cfg(not(windows))]
    fn extensions<'a>(&'a self, _: &'a BTreeMap<String, String>) -> Cow<'a, [OsString]> {
        Cow::Borrowed(&[])
    }

    #[cfg(windows)]
    fn extensions<'a>(&'a self, overrides: &'a BTreeMap<String, String>) -> Cow<'a, [OsString]> {
        environment_value(overrides, "PATHEXT")
            .map(|value| Cow::Owned(path_extensions(Some(OsString::from(value)))))
            .unwrap_or_else(|| Cow::Borrowed(&self.path_ext))
    }
}

fn environment_value<'a>(environment: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
    #[cfg(windows)]
    return environment
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str());

    #[cfg(not(windows))]
    environment.get(name).map(String::as_str)
}

fn env_path(name: &str) -> Vec<PathBuf> {
    env::var_os(name)
        .map(|path| env::split_paths(&path).collect())
        .unwrap_or_default()
}

fn merge_paths(groups: impl IntoIterator<Item = Vec<PathBuf>>) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for group in groups {
        for path in group {
            if path.is_absolute() && !paths.iter().any(|existing| path_eq(existing, &path)) {
                paths.push(path);
            }
        }
    }
    paths
}

#[cfg(windows)]
fn path_eq(left: &Path, right: &Path) -> bool {
    left.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
}

#[cfg(not(windows))]
fn path_eq(left: &Path, right: &Path) -> bool {
    left == right
}

#[cfg(unix)]
async fn discover_unix() -> ExecutionEnvironment {
    let (home, shell) = unix_user();
    let shell_path = match shell {
        Some(shell) => tokio::time::timeout(SHELL_ENV_TIMEOUT, login_shell_path(&shell))
            .await
            .ok()
            .flatten()
            .map(|path| env::split_paths(&path).collect())
            .unwrap_or_default(),
        None => Vec::new(),
    };
    let fallback = unix_fallback_paths(home.as_deref());
    ExecutionEnvironment {
        path: merge_paths([shell_path, env_path("PATH"), fallback]),
    }
}

#[cfg(unix)]
async fn login_shell_path(shell: &Path) -> Option<OsString> {
    let marker = format!("__ONECHAT_PATH_{}__", std::process::id());
    let script = format!("printf '{}%s{}' \"$PATH\"", marker, marker);
    for arguments in [["-ilc", script.as_str()], ["-lc", script.as_str()]] {
        let mut command = tokio::process::Command::new(shell);
        command
            .args(arguments)
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        let output = command.output().await.ok()?;
        if !output.status.success() {
            continue;
        }
        let marker = marker.as_bytes();
        let start = find_bytes(&output.stdout, marker)? + marker.len();
        let end = find_bytes(&output.stdout[start..], marker)? + start;
        return Some(OsString::from_vec(output.stdout[start..end].to_vec()));
    }
    None
}

#[cfg(unix)]
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(unix)]
fn unix_user() -> (Option<PathBuf>, Option<PathBuf>) {
    let mut record = unsafe { std::mem::zeroed::<libc::passwd>() };
    let mut result = std::ptr::null_mut();
    let size = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
    let mut buffer = vec![0_u8; usize::try_from(size).unwrap_or(16_384).max(1024)];
    let status = unsafe {
        libc::getpwuid_r(
            libc::geteuid(),
            &mut record,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    };
    if status != 0 || result.is_null() {
        return (
            env::var_os("HOME").map(PathBuf::from),
            env::var_os("SHELL").map(PathBuf::from),
        );
    }

    let copy_path = |value: *const libc::c_char| {
        (!value.is_null()).then(|| {
            PathBuf::from(OsString::from_vec(
                unsafe { std::ffi::CStr::from_ptr(value) }
                    .to_bytes()
                    .to_vec(),
            ))
        })
    };
    (copy_path(record.pw_dir), copy_path(record.pw_shell))
}

#[cfg(unix)]
fn unix_fallback_paths(home: Option<&Path>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = home {
        paths.push(home.join(".local/bin"));
        paths.push(home.join(".cargo/bin"));
        #[cfg(target_os = "linux")]
        paths.push(home.join(".nix-profile/bin"));
    }
    #[cfg(target_os = "macos")]
    paths.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]);
    #[cfg(target_os = "linux")]
    paths.extend([
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
        PathBuf::from("/snap/bin"),
        PathBuf::from("/nix/var/nix/profiles/default/bin"),
    ]);
    paths
}

#[cfg(windows)]
fn discover_windows() -> ExecutionEnvironment {
    let refreshed = windows_environment();
    let refreshed_path = refreshed
        .as_ref()
        .and_then(|environment| environment_value(environment, "PATH"))
        .map(|path| env::split_paths(path).collect())
        .unwrap_or_default();
    let home = refreshed
        .as_ref()
        .and_then(|environment| environment_value(environment, "USERPROFILE"))
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from));
    let local_app_data = refreshed
        .as_ref()
        .and_then(|environment| environment_value(environment, "LOCALAPPDATA"))
        .map(PathBuf::from)
        .or_else(|| env::var_os("LOCALAPPDATA").map(PathBuf::from));
    let mut fallback = Vec::new();
    if let Some(home) = home {
        fallback.extend([home.join(".local/bin"), home.join(".cargo/bin")]);
    }
    if let Some(local_app_data) = local_app_data {
        fallback.push(local_app_data.join("Microsoft/WindowsApps"));
    }
    let path_ext = refreshed
        .as_ref()
        .and_then(|environment| environment_value(environment, "PATHEXT"))
        .map(OsString::from)
        .or_else(|| env::var_os("PATHEXT"));
    ExecutionEnvironment {
        path: merge_paths([refreshed_path, env_path("PATH"), fallback]),
        path_ext: path_extensions(path_ext),
    }
}

#[cfg(windows)]
fn windows_environment() -> Option<BTreeMap<String, String>> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        Security::{TOKEN_DUPLICATE, TOKEN_QUERY},
        System::{
            Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock},
            Threading::{GetCurrentProcess, OpenProcessToken},
        },
    };

    let mut token: HANDLE = unsafe { std::mem::zeroed() };
    if unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY | TOKEN_DUPLICATE,
            &mut token,
        )
    } == 0
    {
        return None;
    }
    let mut block = std::ptr::null_mut();
    let created = unsafe { CreateEnvironmentBlock(&mut block, token, 1) } != 0;
    unsafe { CloseHandle(token) };
    if !created {
        return None;
    }

    let mut environment = BTreeMap::new();
    let mut cursor = block.cast::<u16>();
    unsafe {
        while *cursor != 0 {
            let mut length = 0;
            while *cursor.add(length) != 0 {
                length += 1;
            }
            let entry = String::from_utf16_lossy(std::slice::from_raw_parts(cursor, length));
            if let Some((name, value)) = entry.split_once('=')
                && !name.is_empty()
            {
                environment.insert(name.to_string(), value.to_string());
            }
            cursor = cursor.add(length + 1);
        }
        DestroyEnvironmentBlock(block);
    }
    Some(environment)
}

#[cfg(windows)]
fn path_extensions(value: Option<OsString>) -> Vec<OsString> {
    let value = value.unwrap_or_else(|| OsString::from(".COM;.EXE;.BAT;.CMD"));
    let mut extensions = vec![OsString::new()];
    extensions.extend(
        value
            .to_string_lossy()
            .split(';')
            .map(str::trim)
            .filter(|extension| !extension.is_empty())
            .map(|extension| {
                if extension.starts_with('.') {
                    OsString::from(extension)
                } else {
                    OsString::from(format!(".{extension}"))
                }
            }),
    );
    extensions
}

#[cfg(windows)]
fn windows_spawnable_extension(extension: &OsStr) -> bool {
    matches!(
        extension
            .to_string_lossy()
            .trim_start_matches('.')
            .to_ascii_uppercase()
            .as_str(),
        "COM" | "EXE" | "BAT" | "CMD"
    )
}

#[cfg(not(windows))]
fn executable_candidate(path: &Path, _: &[OsString]) -> Option<PathBuf> {
    let metadata = fs::metadata(path).ok()?;
    (metadata.is_file() && metadata.permissions().mode() & 0o111 != 0).then(|| absolute_path(path))
}

#[cfg(windows)]
fn executable_candidate(path: &Path, extensions: &[OsString]) -> Option<PathBuf> {
    if path
        .extension()
        .is_some_and(|extension| !windows_spawnable_extension(extension))
    {
        return None;
    }
    let has_extension = path.extension().is_some();
    extensions
        .iter()
        .filter(|extension| {
            (!has_extension || extension.is_empty())
                && (extension.is_empty() || windows_spawnable_extension(extension))
        })
        .map(|extension| {
            if extension.is_empty() {
                path.to_path_buf()
            } else {
                let mut candidate = path.as_os_str().to_os_string();
                candidate.push(extension);
                PathBuf::from(candidate)
            }
        })
        .find(|candidate| fs::metadata(candidate).is_ok_and(|metadata| metadata.is_file()))
        .map(|candidate| absolute_path(&candidate))
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir().unwrap_or_default().join(path)
    }
}
