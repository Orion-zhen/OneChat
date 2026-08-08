use std::{
    env,
    fs,
    io::{self, BufRead, Write},
    thread,
    time::Duration,
};

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let Some(id) = json_id(&line) else { continue };
        let result = if line.contains("\"method\":\"initialize\"") {
            let version = json_string(&line, "protocolVersion")
                .unwrap_or_else(|| "2025-11-25".to_string());
            format!(
                r#"{{"protocolVersion":"{}","capabilities":{{"tools":{{}}}},"serverInfo":{{"name":"onechat-test-server","version":"1.0.0"}}}}"#,
                escape(&version)
            )
        } else if line.contains("\"method\":\"tools/list\"") {
            r#"{"tools":[{"name":"environment","description":"Returns inherited process values","inputSchema":{"type":"object","properties":{}}},{"name":"slow","description":"Waits before responding","inputSchema":{"type":"object","properties":{}}}]}"#.to_string()
        } else if line.contains("\"method\":\"tools/call\"") {
            if line.contains("\"name\":\"slow\"") {
                thread::sleep(Duration::from_millis(500));
            }
            let value = format!(
                "{}|{}",
                env::var("ONECHAT_TEST_VALUE").unwrap_or_default(),
                env::current_dir().unwrap().display()
            );
            format!(
                r#"{{"content":[{{"type":"text","text":"{}"}}],"isError":false}}"#,
                escape(&value)
            )
        } else {
            r#"{"code":-32601,"message":"Method not found"}"#.to_string()
        };
        let response = if line.contains("\"method\":\"unknown\"") {
            format!(r#"{{"jsonrpc":"2.0","id":{id},"error":{result}}}"#)
        } else {
            format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{result}}}"#)
        };
        if writeln!(stdout, "{response}").is_err() || stdout.flush().is_err() {
            break;
        }
    }

    if let Ok(marker) = env::var("ONECHAT_EXIT_MARKER") {
        let _ = fs::write(marker, "exited\n");
    }
}

fn json_id(line: &str) -> Option<&str> {
    let rest = line.split_once("\"id\":")?.1;
    let end = rest.find([',', '}']).unwrap_or(rest.len());
    Some(rest[..end].trim())
}

fn json_string(line: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\":\"");
    let rest = line.split_once(&marker)?.1;
    Some(rest.split_once('"')?.0.to_string())
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
