fn main() {
    #[cfg(target_os = "windows")]
    embed_resource::compile("assets/icons/windows/onechat.rc", embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}
