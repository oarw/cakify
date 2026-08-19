use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use cakify_storage::{
    McpServerStatusUpdate, McpTransport, NewMcpServer, StorageActor, StorageConfig, StorageError,
};

static NEXT_DATABASE: AtomicU64 = AtomicU64::new(1);

struct TestDatabase {
    path: PathBuf,
}

impl TestDatabase {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock")
            .as_nanos();
        let sequence = NEXT_DATABASE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "cakify-mcp-{label}-{}-{nonce}-{sequence}.db",
            std::process::id()
        ));
        Self { path }
    }

    fn config(&self) -> StorageConfig {
        StorageConfig::new(&self.path)
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_file(sidecar_path(&self.path, "-wal"));
        let _ = fs::remove_file(sidecar_path(&self.path, "-shm"));
    }
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

#[test]
fn mcp_servers_persist_with_stable_order_and_optimistic_status_updates() {
    let database = TestDatabase::new("crud");
    {
        let actor = StorageActor::open(database.config()).expect("open actor");
        let handle = actor.handle();
        handle
            .create_mcp_server(NewMcpServer::streamable_http(
                "remote",
                "Zulu",
                "https://mcp.example.invalid/mcp",
                10,
            ))
            .expect("create HTTP server");
        let local = handle
            .create_mcp_server(NewMcpServer::stdio(
                "local",
                "Alpha",
                "C:\\Tools\\mcp-server.exe",
                11,
            ))
            .expect("create stdio server");
        assert_eq!(local.transport, McpTransport::Stdio);
        assert_eq!(local.target().as_deref(), Some("C:\\Tools\\mcp-server.exe"));
        assert!(!local.enabled);

        let enabled = handle
            .set_mcp_server_enabled(McpServerStatusUpdate {
                id: local.id.clone(),
                enabled: true,
                expected_updated_at: local.updated_at,
                updated_at: 12,
            })
            .expect("enable server");
        assert!(enabled.enabled);
        assert!(matches!(
            handle.set_mcp_server_enabled(McpServerStatusUpdate {
                id: local.id,
                enabled: false,
                expected_updated_at: 11,
                updated_at: 13,
            }),
            Err(StorageError::StaleWrite { .. })
        ));
    }

    let actor = StorageActor::open(database.config()).expect("reopen actor");
    let handle = actor.handle();
    let servers = handle.list_mcp_servers().expect("list persisted servers");
    assert_eq!(
        servers
            .iter()
            .map(|server| server.display_name.as_str())
            .collect::<Vec<_>>(),
        vec!["Alpha", "Zulu"]
    );
    assert!(servers[0].enabled);
    handle.delete_mcp_server("local").expect("delete local");
    assert!(handle.get_mcp_server("local").expect("lookup").is_none());
}

#[test]
fn mcp_config_rejects_insecure_remote_urls_and_credential_fields() {
    let database = TestDatabase::new("validation");
    let actor = StorageActor::open(database.config()).expect("open actor");
    let handle = actor.handle();

    assert!(matches!(
        handle.create_mcp_server(NewMcpServer::streamable_http(
            "insecure",
            "Insecure",
            "http://mcp.example.invalid/mcp",
            10,
        )),
        Err(StorageError::InvalidInput { .. })
    ));

    let mut secret =
        NewMcpServer::streamable_http("secret", "Secret", "https://mcp.example.invalid/mcp", 11);
    secret.config_json =
        r#"{"url":"https://mcp.example.invalid/mcp","authorization":"synthetic"}"#.to_owned();
    assert!(matches!(
        handle.create_mcp_server(secret),
        Err(StorageError::SensitiveJsonKey { .. })
    ));
}
