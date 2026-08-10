use std::collections::{BTreeMap, BTreeSet};

use onechat::mcp::{McpConfig, McpManager, McpServerConfig, McpStdioServerConfig};
use tempfile::tempdir;

#[test]
fn mcp_config_accepts_jsonc_defaults_and_rejects_conflicting_auth() {
    let config = McpConfig::parse(
        r#"
        {
          // Both transports can coexist.
          "mcpServers": {
            "local": { "command": "  npx  ", "args": ["server"] },
            "remote": { "url": "https://example.com/mcp" },
          },
        }
        "#,
    )
    .unwrap();

    assert_eq!(config.servers.len(), 2);
    let McpServerConfig::Stdio(local) = &config.servers["local"] else {
        panic!("expected stdio config");
    };
    assert!(local.enabled);
    assert_eq!(local.command, "npx");
    assert!(config.servers["remote"].enabled());

    let error = McpConfig::parse(
        r#"{
          "mcpServers": {
            "remote": {
              "url": "https://example.com/mcp",
              "bearerToken": "token",
              "headers": { "Authorization": "Bearer another-token" }
            }
          }
        }"#,
    )
    .unwrap_err();
    assert!(error.to_string().contains("must use only one"));
}

#[tokio::test]
async fn mcp_manager_edits_config_without_discarding_comments() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("mcp.jsonc");
    std::fs::write(
        &path,
        r#"{
  // keep this comment
  "mcpServers": {
    "local": {
      "enabled": false,
      "command": "old-command",
      "args": [],
    },
  },
}
"#,
    )
    .unwrap();
    let manager = McpManager::new(&path);

    manager
        .upsert_server(
            "local".into(),
            McpServerConfig::Stdio(McpStdioServerConfig {
                enabled: false,
                command: "new-command".into(),
                args: vec!["--stdio".into()],
                env: BTreeMap::from([("TOKEN".into(), "value".into())]),
                cwd: None,
                disabled_tools: BTreeSet::from(["write".into()]),
            }),
        )
        .await
        .unwrap();
    let (count, snapshot) = manager
        .import_servers(
            r#"{
              "mcpServers": {
                "remote": { "enabled": false, "url": "https://example.com/mcp" }
              }
            }"#
            .into(),
        )
        .await
        .unwrap();

    assert_eq!(count, 1);
    assert_eq!(snapshot.servers.len(), 2);
    assert!(snapshot.servers.iter().all(|server| !server.enabled));
    let source = std::fs::read_to_string(&path).unwrap();
    assert!(source.contains("// keep this comment"));
    let config = McpConfig::load(&path).unwrap();
    let McpServerConfig::Stdio(local) = &config.servers["local"] else {
        panic!("expected stdio config");
    };
    assert_eq!(local.command, "new-command");
    assert_eq!(local.args, vec!["--stdio"]);
    assert!(local.disabled_tools.contains("write"));

    let snapshot = manager.delete_server("remote".into()).await.unwrap();
    assert_eq!(snapshot.servers.len(), 1);
    assert!(
        !McpConfig::load(&path)
            .unwrap()
            .servers
            .contains_key("remote")
    );
    manager.shutdown().await;
}
