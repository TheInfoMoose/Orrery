use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::config::{
    McpServerConfig, McpTransportConfig, RoleConfig, WriteToolSpec, format_mapped_tool_name,
    validate_concrete_tool_ref, validate_safe_relative_path,
};
use crate::security::SecretStore;
use crate::stream_log::{LiveStreamHub, StreamEvent};
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpToolSchema {
    pub server_id: String,
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn list_tools(&self) -> Result<Vec<McpToolSchema>>;
    async fn call_tool(&self, name: &str, args: serde_json::Value) -> Result<serde_json::Value>;
}

#[async_trait]
pub trait McpConnector: Send + Sync {
    async fn connect(&self, server: &McpServerConfig) -> Result<Arc<dyn McpTransport>>;
    fn attach_stream(&self, _stream: Arc<LiveStreamHub>) {}
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WriteScope {
    prefixes: Vec<String>,
}

impl WriteScope {
    pub fn new(prefixes: Vec<String>) -> Result<Self> {
        for prefix in &prefixes {
            validate_safe_relative_path(Path::new(prefix), "write_scope")?;
        }
        Ok(Self { prefixes })
    }

    pub fn anywhere() -> Self {
        Self {
            prefixes: Vec::new(),
        }
    }

    pub fn prefixes(&self) -> &[String] {
        &self.prefixes
    }

    pub fn contains(&self, path: &str) -> bool {
        let Some(path_segs) = normalize_path_segments(path) else {
            return false;
        };
        if self.prefixes.is_empty() {
            return true;
        }
        self.prefixes.iter().any(|prefix| {
            normalize_path_segments(prefix)
                .is_some_and(|prefix_segs| path_segs.starts_with(&prefix_segs))
        })
    }
}

fn normalize_path_segments(path: &str) -> Option<Vec<&str>> {
    if path.starts_with('/') {
        return None;
    }
    let mut segments = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => return None,
            other => segments.push(other),
        }
    }
    if segments.is_empty() {
        return None;
    }
    Some(segments)
}

pub fn scopes_overlap(a: &str, b: &str) -> bool {
    let (Some(a_segs), Some(b_segs)) = (normalize_path_segments(a), normalize_path_segments(b))
    else {
        return true;
    };
    a_segs.starts_with(&b_segs) || b_segs.starts_with(&a_segs)
}

pub fn tool_result_is_error(result: &serde_json::Value) -> bool {
    result
        .get("isError")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

pub fn tool_result_text(result: &serde_json::Value) -> String {
    result
        .get("content")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("text").and_then(serde_json::Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

#[derive(Default)]
pub struct McpGatekeeper {
    transports: HashMap<String, Arc<dyn McpTransport>>,
    write_tools: HashMap<String, Vec<WriteToolSpec>>,
    catalog: Vec<McpToolSchema>,
}

impl McpGatekeeper {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        server_id: impl Into<String>,
        transport: Arc<dyn McpTransport>,
        write_tools: Vec<WriteToolSpec>,
    ) {
        let server_id = server_id.into();
        self.transports.insert(server_id.clone(), transport);
        self.write_tools.insert(server_id, write_tools);
    }

    pub async fn refresh_catalog(&mut self) -> Result<()> {
        let mut server_ids: Vec<String> = self.transports.keys().cloned().collect();
        server_ids.sort();

        let mut next_catalog = Vec::new();
        for server_id in server_ids {
            if let Some(transport) = self.transports.get(&server_id) {
                let mut tools = transport.list_tools().await?;
                for tool in &mut tools {
                    if tool.server_id.is_empty() {
                        tool.server_id = server_id.clone();
                    }
                }
                next_catalog.extend(tools);
            }
        }
        self.catalog = next_catalog;
        Ok(())
    }

    pub fn catalog(&self) -> &[McpToolSchema] {
        &self.catalog
    }

    pub fn is_tool_allowed(role: &RoleConfig, server_id: &str, tool: &str) -> bool {
        if format_mapped_tool_name(server_id, tool).is_err() {
            return false;
        }

        for pattern in &role.allowed_mcp_tools {
            if let Some(pat_server) = pattern.strip_suffix(":*") {
                if pat_server != "*" && pat_server == server_id {
                    return true;
                }
            } else if let Ok((pat_server, pat_tool)) =
                validate_concrete_tool_ref(pattern, "allowed_mcp_tools")
                && pat_server == server_id
                && pat_tool == tool
            {
                return true;
            }
        }

        false
    }

    pub fn is_write_tool(&self, server_id: &str, tool: &str) -> bool {
        self.write_tools
            .get(server_id)
            .is_some_and(|specs| specs.iter().any(|spec| spec.tool == tool))
    }

    pub fn tools_for_role(&self, role: &RoleConfig) -> Vec<McpToolSchema> {
        self.catalog
            .iter()
            .filter(|schema| Self::is_tool_allowed(role, &schema.server_id, &schema.name))
            .cloned()
            .collect()
    }

    pub async fn execute_for_role(
        &self,
        role: &RoleConfig,
        scope: &WriteScope,
        server_id: &str,
        tool: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value> {
        if !args.is_object() {
            return Err(Error::Security {
                message: format!("tool '{server_id}:{tool}' arguments must be a JSON object"),
            });
        }

        if !Self::is_tool_allowed(role, server_id, tool) {
            return Err(Error::Security {
                message: format!(
                    "role '{}' is not allowed to call tool '{server_id}:{tool}'",
                    role.role_id
                ),
            });
        }

        let Some(transport) = self.transports.get(server_id) else {
            return Err(Error::Security {
                message: format!("MCP server '{server_id}' is not registered"),
            });
        };

        if let Some(specs) = self.write_tools.get(server_id) {
            for spec in specs.iter().filter(|spec| spec.tool == tool) {
                for arg_name in &spec.path_args {
                    if let Some(val) = args.get(arg_name) {
                        let Some(path_str) = val.as_str() else {
                            return Err(Error::Security {
                                message: format!(
                                    "write tool '{server_id}:{tool}' argument '{arg_name}' must be a JSON string"
                                ),
                            });
                        };
                        if !scope.contains(path_str) {
                            return Err(Error::Security {
                                message: format!(
                                    "write tool '{server_id}:{tool}' argument '{arg_name}' path '{path_str}' is outside write scope"
                                ),
                            });
                        }
                    }
                }
            }
        }

        transport.call_tool(tool, args).await
    }
}

#[derive(Debug, Clone)]
enum MockToolOutcome {
    Success(serde_json::Value),
    Failure(String),
}

pub struct MockMcpTransport {
    server_id: String,
    tools: Vec<McpToolSchema>,
    results: HashMap<String, MockToolOutcome>,
    calls: Mutex<Vec<(String, serde_json::Value)>>,
}

impl MockMcpTransport {
    pub fn new(server_id: impl Into<String>) -> Self {
        Self {
            server_id: server_id.into(),
            tools: Vec::new(),
            results: HashMap::new(),
            calls: Mutex::new(Vec::new()),
        }
    }

    pub fn with_tool(mut self, name: &str, result: serde_json::Value) -> Self {
        self.register_schema(name);
        self.results
            .insert(name.to_string(), MockToolOutcome::Success(result));
        self
    }

    pub fn with_failing_tool(mut self, name: &str, message: &str) -> Self {
        self.register_schema(name);
        self.results.insert(
            name.to_string(),
            MockToolOutcome::Failure(message.to_string()),
        );
        self
    }

    pub fn calls(&self) -> Vec<(String, serde_json::Value)> {
        match self.calls.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn register_schema(&mut self, name: &str) {
        if !self.tools.iter().any(|t| t.name == name) {
            self.tools.push(McpToolSchema {
                server_id: self.server_id.clone(),
                name: name.to_string(),
                description: "mock".to_string(),
                input_schema: serde_json::json!({"type": "object"}),
            });
        }
    }
}

#[async_trait]
impl McpTransport for MockMcpTransport {
    async fn list_tools(&self) -> Result<Vec<McpToolSchema>> {
        Ok(self.tools.clone())
    }

    async fn call_tool(&self, name: &str, args: serde_json::Value) -> Result<serde_json::Value> {
        self.calls
            .lock()
            .map_err(|_| Error::Security {
                message: "MockMcpTransport lock poisoned".into(),
            })?
            .push((name.to_string(), args));

        match self.results.get(name) {
            Some(MockToolOutcome::Success(val)) => Ok(val.clone()),
            Some(MockToolOutcome::Failure(message)) => Err(Error::Mcp {
                server_id: self.server_id.clone(),
                source: Box::new(std::io::Error::other(message.clone())),
            }),
            None => Err(Error::Mcp {
                server_id: self.server_id.clone(),
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("unknown mock tool '{name}'"),
                )),
            }),
        }
    }
}

fn is_executable_regular_file(meta: &std::fs::Metadata) -> bool {
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

pub fn resolve_stdio_command(
    command: &str,
    project_root: &Path,
    search_path: Option<&OsStr>,
) -> Result<PathBuf> {
    if command.trim().is_empty() {
        return Err(Error::Validation {
            field: "transport.command".to_string(),
            message: "command must not be empty".to_string(),
        });
    }

    let canonical_cmd = if !command.contains('/') && !command.contains('\\') {
        let mut found = None;
        if let Some(paths) = search_path {
            for entry in std::env::split_paths(paths) {
                if entry.as_os_str().is_empty() || !entry.is_absolute() {
                    continue;
                }
                let candidate = entry.join(command);
                if let Ok(meta) = std::fs::metadata(&candidate)
                    && is_executable_regular_file(&meta)
                {
                    let canonical =
                        std::fs::canonicalize(&candidate).map_err(|source| Error::Io {
                            path: candidate,
                            source,
                        })?;
                    found = Some(canonical);
                    break;
                }
            }
        }
        match found {
            Some(path) => path,
            None => {
                return Err(Error::Validation {
                    field: "transport.command".to_string(),
                    message: format!("'{command}' not found on PATH"),
                });
            }
        }
    } else {
        let cmd_path = Path::new(command);
        if !cmd_path.is_absolute() {
            return Err(Error::Validation {
                field: "transport.command".to_string(),
                message: format!("relative stdio command '{command}' is not allowed"),
            });
        }
        let canonical = std::fs::canonicalize(cmd_path).map_err(|source| Error::Io {
            path: cmd_path.to_path_buf(),
            source,
        })?;
        let meta = std::fs::metadata(&canonical).map_err(|source| Error::Io {
            path: canonical.clone(),
            source,
        })?;
        if !is_executable_regular_file(&meta) {
            return Err(Error::Validation {
                field: "transport.command".to_string(),
                message: format!(
                    "stdio command '{}' must be an executable regular file",
                    canonical.display()
                ),
            });
        }
        canonical
    };

    let canonical_root = std::fs::canonicalize(project_root).map_err(|source| Error::Io {
        path: project_root.to_path_buf(),
        source,
    })?;
    let canonical_orrery = canonical_root.join(".orrery");
    if canonical_cmd.starts_with(&canonical_root) && !canonical_cmd.starts_with(&canonical_orrery) {
        return Err(Error::Security {
            message: "stdio command is inside the agent-writable workspace".to_string(),
        });
    }

    Ok(canonical_cmd)
}

pub struct RmcpTransport {
    server_id: String,
    service: rmcp::service::RunningService<rmcp::RoleClient, ()>,
}

impl RmcpTransport {
    pub async fn connect(server: &McpServerConfig, secrets: &dyn SecretStore) -> Result<Self> {
        Self::connect_in(server, secrets, Path::new(".")).await
    }

    pub async fn connect_in(
        server: &McpServerConfig,
        secrets: &dyn SecretStore,
        workdir: &Path,
    ) -> Result<Self> {
        Self::connect_with_stream(server, secrets, workdir, None).await
    }

    pub async fn connect_with_stream(
        server: &McpServerConfig,
        secrets: &dyn SecretStore,
        workdir: &Path,
        stream: Option<Arc<LiveStreamHub>>,
    ) -> Result<Self> {
        let server_id = server.server_id.clone();
        let service = match &server.transport {
            McpTransportConfig::Stdio { command, args } => {
                let mut cmd = tokio::process::Command::new(command);
                cmd.args(args).current_dir(workdir);
                let transport = match stream {
                    None => {
                        rmcp::transport::TokioChildProcess::new(cmd).map_err(|e| Error::Mcp {
                            server_id: server_id.clone(),
                            source: Box::new(e),
                        })?
                    }
                    Some(hub) => {
                        let (transport, stderr) = rmcp::transport::TokioChildProcess::builder(cmd)
                            .stderr(std::process::Stdio::piped())
                            .spawn()
                            .map_err(|e| Error::Mcp {
                                server_id: server_id.clone(),
                                source: Box::new(e),
                            })?;
                        if let Some(handle) = stderr {
                            spawn_stderr_forwarder(server_id.clone(), handle, hub);
                        }
                        transport
                    }
                };
                rmcp::ServiceExt::serve((), transport)
                    .await
                    .map_err(|e| Error::Mcp {
                        server_id: server_id.clone(),
                        source: Box::new(e),
                    })?
            }
            McpTransportConfig::StreamableHttp {
                endpoint_url,
                keychain_service,
                keychain_account,
            } => {
                let mut cfg =
                    rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig::with_uri(
                        endpoint_url.clone(),
                    );
                match (keychain_service.as_deref(), keychain_account.as_deref()) {
                    (Some(svc), Some(acc)) => {
                        let secret = secrets.get(svc, acc)?;
                        cfg = cfg.auth_header(secret.expose());
                    }
                    (None, None) => {}
                    _ => {
                        return Err(Error::Validation {
                            field: "transport.keychain_service".to_string(),
                            message: "keychain_service and keychain_account must both be set or both omitted"
                                .to_string(),
                        });
                    }
                }
                let transport = rmcp::transport::StreamableHttpClientTransport::from_config(cfg);
                rmcp::ServiceExt::serve((), transport)
                    .await
                    .map_err(|e| Error::Mcp {
                        server_id: server_id.clone(),
                        source: Box::new(e),
                    })?
            }
        };

        Ok(Self { server_id, service })
    }
}

pub fn spawn_stderr_forwarder<R>(
    server_id: String,
    stderr: R,
    stream: Arc<LiveStreamHub>,
) -> tokio::task::JoinHandle<()>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;
        let mut lines = tokio::io::BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if !line.trim().is_empty() {
                let _ = stream.emit(StreamEvent::notice(format!("[mcp:{server_id}] {line}")));
            }
        }
    })
}

#[async_trait]
impl McpTransport for RmcpTransport {
    async fn list_tools(&self) -> Result<Vec<McpToolSchema>> {
        let tools = self
            .service
            .peer()
            .list_all_tools()
            .await
            .map_err(|e| Error::Mcp {
                server_id: self.server_id.clone(),
                source: Box::new(e),
            })?;

        Ok(tools
            .into_iter()
            .map(|tool| McpToolSchema {
                server_id: self.server_id.clone(),
                name: tool.name.into_owned(),
                description: tool
                    .description
                    .map(std::borrow::Cow::into_owned)
                    .unwrap_or_default(),
                input_schema: serde_json::Value::Object(tool.input_schema.as_ref().clone()),
            })
            .collect())
    }

    async fn call_tool(&self, name: &str, args: serde_json::Value) -> Result<serde_json::Value> {
        let serde_json::Value::Object(obj) = args else {
            return Err(Error::Validation {
                field: "args".to_string(),
                message: "tool arguments must be a JSON object".to_string(),
            });
        };
        let params = rmcp::model::CallToolRequestParams::new(name.to_owned()).with_arguments(obj);
        let result = self
            .service
            .peer()
            .call_tool(params)
            .await
            .map_err(|e| Error::Mcp {
                server_id: self.server_id.clone(),
                source: Box::new(e),
            })?;
        Ok(serde_json::to_value(result)?)
    }
}

pub struct RmcpConnector {
    pub secrets: Arc<dyn SecretStore>,
    pub project_root: PathBuf,
    pub search_path: Option<OsString>,
    pub stream: std::sync::OnceLock<Arc<LiveStreamHub>>,
}

#[async_trait]
impl McpConnector for RmcpConnector {
    async fn connect(&self, server: &McpServerConfig) -> Result<Arc<dyn McpTransport>> {
        let transport = match &server.transport {
            McpTransportConfig::Stdio { command, args } => {
                let resolved = resolve_stdio_command(
                    command,
                    &self.project_root,
                    self.search_path.as_deref(),
                )?;
                let command_str =
                    resolved
                        .into_os_string()
                        .into_string()
                        .map_err(|_| Error::Validation {
                            field: "transport.command".to_string(),
                            message: "resolved stdio command path is not valid UTF-8".to_string(),
                        })?;
                let mut clone = server.clone();
                clone.transport = McpTransportConfig::Stdio {
                    command: command_str,
                    args: args.clone(),
                };
                RmcpTransport::connect_with_stream(
                    &clone,
                    self.secrets.as_ref(),
                    &self.project_root,
                    self.stream.get().cloned(),
                )
                .await?
            }
            McpTransportConfig::StreamableHttp { .. } => {
                RmcpTransport::connect(server, self.secrets.as_ref()).await?
            }
        };
        Ok(Arc::new(transport))
    }

    fn attach_stream(&self, stream: Arc<LiveStreamHub>) {
        let _ = self.stream.set(stream);
    }
}

#[derive(Default)]
pub struct MockMcpConnector {
    pub transports: HashMap<String, Arc<dyn McpTransport>>,
}

#[async_trait]
impl McpConnector for MockMcpConnector {
    async fn connect(&self, server: &McpServerConfig) -> Result<Arc<dyn McpTransport>> {
        self.transports
            .get(&server.server_id)
            .cloned()
            .ok_or_else(|| Error::Mcp {
                server_id: server.server_id.clone(),
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("unknown MCP server '{}'", server.server_id),
                )),
            })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::error::Error as StdError;
    use std::sync::Arc;

    use serde_json::json;

    use super::{
        McpConnector, McpGatekeeper, McpTransport, MockMcpConnector, MockMcpTransport,
        RmcpConnector, RmcpTransport, WriteScope, resolve_stdio_command, scopes_overlap,
        spawn_stderr_forwarder, tool_result_is_error, tool_result_text,
    };
    use crate::Error;
    use crate::config::{McpServerConfig, McpTransportConfig, RoleConfig, RoleKind, WriteToolSpec};
    use crate::security::{InMemorySecretStore, SecretRedactor, SecretValue};
    use crate::stream_log::{LiveStreamConfig, LiveStreamHub, StreamEventKind};

    fn test_role(allowed_mcp_tools: Vec<&str>) -> RoleConfig {
        RoleConfig {
            role_id: "coder".to_string(),
            display_name: "Coder".to_string(),
            kind: RoleKind::Implementer,
            rules_file: "roles/coder.md".to_string(),
            allowed_mcp_tools: allowed_mcp_tools.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn exact_pattern_allows_only_that_tool() {
        let role = test_role(vec!["fs:read_file"]);
        assert!(McpGatekeeper::is_tool_allowed(&role, "fs", "read_file"));
        assert!(!McpGatekeeper::is_tool_allowed(&role, "fs", "write_file"));
        assert!(!McpGatekeeper::is_tool_allowed(&role, "git", "read_file"));
        assert!(!McpGatekeeper::is_tool_allowed(
            &role,
            "bad__id",
            "read_file"
        ));
    }

    #[test]
    fn server_wildcard_allows_server_tools() {
        let role = test_role(vec!["fs:*"]);
        assert!(McpGatekeeper::is_tool_allowed(&role, "fs", "read_file"));
        assert!(McpGatekeeper::is_tool_allowed(&role, "fs", "write_file"));
        assert!(!McpGatekeeper::is_tool_allowed(&role, "git", "status"));
    }

    #[test]
    fn star_star_not_honored() {
        let role = test_role(vec!["*:*", "*:read_file", "invalid_pattern"]);
        assert!(!McpGatekeeper::is_tool_allowed(&role, "fs", "read_file"));
        assert!(!McpGatekeeper::is_tool_allowed(&role, "*", "read_file"));
    }

    #[tokio::test]
    async fn disallowed_call_is_security_error_and_never_reaches_transport() {
        let role = test_role(vec!["fs:read_file"]);
        let mock = Arc::new(
            MockMcpTransport::new("fs")
                .with_tool("read_file", json!({"ok": true}))
                .with_tool("write_file", json!({"ok": true})),
        );
        let mut gatekeeper = McpGatekeeper::new();
        gatekeeper.register("fs", mock.clone(), Vec::new());

        let err = gatekeeper
            .execute_for_role(
                &role,
                &WriteScope::anywhere(),
                "fs",
                "write_file",
                json!({"path": "src/lib.rs"}),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, Error::Security { .. }));
        assert!(mock.calls().is_empty());
    }

    #[tokio::test]
    async fn unregistered_server_is_security_error() {
        let role = test_role(vec!["fs:read_file"]);
        let gatekeeper = McpGatekeeper::new();

        let err = gatekeeper
            .execute_for_role(
                &role,
                &WriteScope::anywhere(),
                "fs",
                "read_file",
                json!({"path": "src/lib.rs"}),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, Error::Security { .. }));
    }

    #[tokio::test]
    async fn write_tool_path_outside_scope_rejected() {
        let role = test_role(vec!["fs:write_file"]);
        let mock =
            Arc::new(MockMcpTransport::new("fs").with_tool("write_file", json!({"ok": true})));
        let mut gatekeeper = McpGatekeeper::new();
        gatekeeper.register(
            "fs",
            mock.clone(),
            vec![WriteToolSpec {
                tool: "write_file".to_string(),
                path_args: vec!["path".to_string()],
            }],
        );
        assert!(gatekeeper.is_write_tool("fs", "write_file"));
        assert!(!gatekeeper.is_write_tool("fs", "read_file"));

        let scope = WriteScope::new(vec!["src".to_string()]).unwrap();
        assert_eq!(scope.prefixes(), &["src".to_string()]);
        let err = gatekeeper
            .execute_for_role(
                &role,
                &scope,
                "fs",
                "write_file",
                json!({"path": "docs/a.md"}),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, Error::Security { .. }));
        assert!(mock.calls().is_empty());
    }

    #[tokio::test]
    async fn write_tool_path_inside_scope_allowed() {
        let role = test_role(vec!["fs:write_file"]);
        let mock =
            Arc::new(MockMcpTransport::new("fs").with_tool("write_file", json!({"ok": true})));
        let mut gatekeeper = McpGatekeeper::new();
        gatekeeper.register(
            "fs",
            mock.clone(),
            vec![WriteToolSpec {
                tool: "write_file".to_string(),
                path_args: vec!["path".to_string()],
            }],
        );

        let scope = WriteScope::new(vec!["src".to_string()]).unwrap();
        let out = gatekeeper
            .execute_for_role(
                &role,
                &scope,
                "fs",
                "write_file",
                json!({"path": "src/lib.rs"}),
            )
            .await
            .unwrap();

        assert_eq!(out, json!({"ok": true}));
        assert_eq!(
            mock.calls(),
            vec![("write_file".to_string(), json!({"path": "src/lib.rs"}))]
        );
    }

    #[tokio::test]
    async fn write_tool_dotdot_rejected() {
        let role = test_role(vec!["fs:write_file"]);
        let mock =
            Arc::new(MockMcpTransport::new("fs").with_tool("write_file", json!({"ok": true})));
        let mut gatekeeper = McpGatekeeper::new();
        gatekeeper.register(
            "fs",
            mock.clone(),
            vec![WriteToolSpec {
                tool: "write_file".to_string(),
                path_args: vec!["path".to_string()],
            }],
        );

        let scope = WriteScope::new(vec!["src".to_string()]).unwrap();
        let err = gatekeeper
            .execute_for_role(
                &role,
                &scope,
                "fs",
                "write_file",
                json!({"path": "src/../etc/x"}),
            )
            .await
            .unwrap_err();

        assert!(matches!(err, Error::Security { .. }));
        assert!(mock.calls().is_empty());
        assert!(WriteScope::new(vec!["../etc".to_string()]).is_err());
    }

    #[tokio::test]
    async fn write_tool_non_string_path_rejected() {
        let role = test_role(vec!["fs:write_file"]);
        let mock =
            Arc::new(MockMcpTransport::new("fs").with_tool("write_file", json!({"ok": true})));
        let mut gatekeeper = McpGatekeeper::new();
        gatekeeper.register(
            "fs",
            mock.clone(),
            vec![WriteToolSpec {
                tool: "write_file".to_string(),
                path_args: vec!["path".to_string()],
            }],
        );

        let scope = WriteScope::new(vec!["src".to_string()]).unwrap();
        let err = gatekeeper
            .execute_for_role(&role, &scope, "fs", "write_file", json!({"path": 42}))
            .await
            .unwrap_err();

        assert!(matches!(err, Error::Security { .. }));
        assert!(mock.calls().is_empty());
    }

    #[tokio::test]
    async fn non_write_tool_ignores_scope() {
        let role = test_role(vec!["fs:read_file"]);
        let mock =
            Arc::new(MockMcpTransport::new("fs").with_tool("read_file", json!({"data": "abc"})));
        let mut gatekeeper = McpGatekeeper::new();
        gatekeeper.register(
            "fs",
            mock.clone(),
            vec![WriteToolSpec {
                tool: "write_file".to_string(),
                path_args: vec!["path".to_string()],
            }],
        );

        let scope = WriteScope::new(vec!["src".to_string()]).unwrap();
        let out = gatekeeper
            .execute_for_role(
                &role,
                &scope,
                "fs",
                "read_file",
                json!({"path": "docs/readme.md"}),
            )
            .await
            .unwrap();

        assert_eq!(out, json!({"data": "abc"}));
        assert_eq!(mock.calls().len(), 1);
    }

    #[test]
    fn anywhere_scope_contains_relative_paths() {
        let scope = WriteScope::anywhere();
        assert!(scope.contains("src/lib.rs"));
        assert!(scope.contains("Cargo.toml"));
        assert!(scope.contains("./crates/orrery-core/src/mcp.rs"));
        assert!(!scope.contains("src/../secret.txt"));
        assert!(!scope.contains(""));
    }

    #[test]
    fn anywhere_scope_rejects_absolute() {
        let scope = WriteScope::anywhere();
        assert!(!scope.contains("/workspace/src/lib.rs"));
        assert!(!scope.contains("/"));
    }

    #[test]
    fn scopes_overlap_cases() {
        assert!(scopes_overlap("src", "src/lib.rs"));
        assert!(!scopes_overlap("src", "src2"));
        assert!(scopes_overlap("./src/", "src"));
        assert!(scopes_overlap("src/a/b", "src/a"));
        assert!(scopes_overlap("../escape", "src"));
        assert!(scopes_overlap("/abs/path", "src"));
    }

    #[tokio::test]
    async fn tools_for_role_filters_catalog() {
        let fs_mock = Arc::new(
            MockMcpTransport::new("fs")
                .with_tool("read_file", json!({}))
                .with_tool("write_file", json!({})),
        );
        let git_mock = Arc::new(MockMcpTransport::new("git").with_tool("status", json!({})));

        let mut gatekeeper = McpGatekeeper::new();
        gatekeeper.register("fs", fs_mock, Vec::new());
        gatekeeper.register("git", git_mock, Vec::new());
        gatekeeper.refresh_catalog().await.unwrap();

        let role = test_role(vec!["fs:read_file", "git:*"]);
        let tools = gatekeeper.tools_for_role(&role);
        let names: Vec<(&str, &str)> = tools
            .iter()
            .map(|t| (t.server_id.as_str(), t.name.as_str()))
            .collect();
        assert_eq!(names, vec![("fs", "read_file"), ("git", "status")]);
    }

    #[test]
    fn tool_result_helpers() {
        let err_result = json!({
            "isError": true,
            "content": [
                {"type": "text", "text": "line 1"},
                {"type": "image", "data": "ignored"},
                {"type": "text", "text": "line 2"}
            ]
        });
        assert!(tool_result_is_error(&err_result));
        assert_eq!(tool_result_text(&err_result), "line 1\nline 2");

        let ok_result = json!({
            "isError": false,
            "content": [{"type": "text", "text": "single line"}]
        });
        assert!(!tool_result_is_error(&ok_result));
        assert_eq!(tool_result_text(&ok_result), "single line");
        assert!(!tool_result_is_error(&json!({})));
        assert_eq!(tool_result_text(&json!({})), "");
    }

    #[tokio::test]
    async fn failing_tool_returns_mcp_error() {
        let role = test_role(vec!["fs:*"]);
        let mock = Arc::new(MockMcpTransport::new("fs").with_failing_tool("boom", "disk full"));
        let mut gatekeeper = McpGatekeeper::new();
        gatekeeper.register("fs", mock.clone(), Vec::new());

        let err = gatekeeper
            .execute_for_role(&role, &WriteScope::anywhere(), "fs", "boom", json!({}))
            .await
            .unwrap_err();

        match &err {
            Error::Mcp { server_id, source } => {
                assert_eq!(server_id, "fs");
                assert_eq!(source.to_string(), "disk full");
            }
            other => panic!("expected Error::Mcp, got {other:?}"),
        }
        assert!(err.source().is_some());

        let missing_err = mock.call_tool("no_such_tool", json!({})).await.unwrap_err();
        assert!(matches!(missing_err, Error::Mcp { .. }));
    }

    #[tokio::test]
    async fn mock_connector_unknown_server_errors() {
        let known_transport: Arc<dyn McpTransport> =
            Arc::new(MockMcpTransport::new("fs").with_tool("read_file", json!({})));
        let mut transports = HashMap::new();
        transports.insert("fs".to_string(), known_transport);
        let connector = MockMcpConnector { transports };

        let known_cfg = McpServerConfig {
            server_id: "fs".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "echo".to_string(),
                args: Vec::new(),
            },
            write_tools: Vec::new(),
        };
        assert!(connector.connect(&known_cfg).await.is_ok());

        let unknown_cfg = McpServerConfig {
            server_id: "unknown".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "echo".to_string(),
                args: Vec::new(),
            },
            write_tools: Vec::new(),
        };
        let err = match connector.connect(&unknown_cfg).await {
            Ok(_) => panic!("expected unknown server error"),
            Err(err) => err,
        };
        match err {
            Error::Mcp { server_id, .. } => assert_eq!(server_id, "unknown"),
            other => panic!("expected Error::Mcp, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn missing_http_secret_is_keychain_error_before_network() {
        let server = McpServerConfig {
            server_id: "remote".to_string(),
            transport: McpTransportConfig::StreamableHttp {
                endpoint_url: "https://mcp.example.com/mcp".to_string(),
                keychain_service: Some("orrery.mcp".to_string()),
                keychain_account: Some("testuser".to_string()),
            },
            write_tools: Vec::new(),
        };
        let secrets = InMemorySecretStore::new();
        let err = match RmcpTransport::connect(&server, &secrets).await {
            Ok(_) => panic!("expected Error::Keychain for missing HTTP secret"),
            Err(err) => err,
        };
        assert!(
            matches!(err, Error::Keychain(_)),
            "expected Error::Keychain, got {err:?}"
        );

        let dir = tempfile::tempdir().unwrap();
        let connector = RmcpConnector {
            secrets: Arc::new(InMemorySecretStore::new()),
            project_root: dir.path().to_path_buf(),
            search_path: None,
            stream: std::sync::OnceLock::new(),
        };
        let conn_err = match connector.connect(&server).await {
            Ok(_) => panic!("expected Error::Keychain from RmcpConnector"),
            Err(err) => err,
        };
        assert!(matches!(conn_err, Error::Keychain(_)));
    }

    #[tokio::test]
    async fn stdio_missing_command_errors() {
        let server = McpServerConfig {
            server_id: "missing_bin".to_string(),
            transport: McpTransportConfig::Stdio {
                command: "orrery-test-no-such-binary".to_string(),
                args: vec!["--flag".to_string()],
            },
            write_tools: Vec::new(),
        };
        let secrets = InMemorySecretStore::new();
        let res = RmcpTransport::connect(&server, &secrets).await;
        assert!(res.is_err(), "missing binary should return Err");

        let dir = tempfile::tempdir().unwrap();
        let connector = RmcpConnector {
            secrets: Arc::new(InMemorySecretStore::new()),
            project_root: dir.path().to_path_buf(),
            search_path: None,
            stream: std::sync::OnceLock::new(),
        };
        let conn_res = connector.connect(&server).await;
        assert!(matches!(conn_res, Err(Error::Validation { .. })));
    }

    #[tokio::test]
    async fn non_object_args_rejected_before_transport() {
        let role = test_role(vec!["fs:write_file"]);
        let mock =
            Arc::new(MockMcpTransport::new("fs").with_tool("write_file", json!({"ok": true})));
        let mut gatekeeper = McpGatekeeper::new();
        gatekeeper.register(
            "fs",
            mock.clone(),
            vec![WriteToolSpec {
                tool: "write_file".to_string(),
                path_args: vec!["path".to_string()],
            }],
        );
        let scope = WriteScope::new(vec!["src".to_string()]).unwrap();

        let err_array = gatekeeper
            .execute_for_role(&role, &scope, "fs", "write_file", json!(["src/lib.rs"]))
            .await
            .unwrap_err();
        assert!(matches!(err_array, Error::Security { .. }));

        let err_string = gatekeeper
            .execute_for_role(&role, &scope, "fs", "write_file", json!("src/lib.rs"))
            .await
            .unwrap_err();
        assert!(matches!(err_string, Error::Security { .. }));

        assert!(mock.calls().is_empty());
    }

    fn write_executable(path: &std::path::Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, b"#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(path, perms).unwrap();
        }
    }

    #[test]
    fn resolve_stdio_bare_name_on_path() {
        let tmp = tempfile::tempdir().unwrap();
        let project_root = tmp.path().join("project");
        std::fs::create_dir_all(&project_root).unwrap();

        let bin_dir = tmp.path().join("bin");
        let fake_mcp = bin_dir.join("fake-mcp");
        write_executable(&fake_mcp);

        let search_path = std::env::join_paths([&bin_dir]).unwrap();
        let resolved =
            resolve_stdio_command("fake-mcp", &project_root, Some(&search_path)).unwrap();
        let expected = std::fs::canonicalize(&fake_mcp).unwrap();
        assert_eq!(resolved, expected);
    }

    #[test]
    fn resolve_stdio_only_relative_entries_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let bin_dir = tmp.path().join("bin");
        write_executable(&bin_dir.join("fake-mcp"));

        let search_path = std::env::join_paths(["bin", ""]).unwrap();
        let err = resolve_stdio_command("fake-mcp", tmp.path(), Some(&search_path)).unwrap_err();
        match err {
            Error::Validation { field, message } => {
                assert_eq!(field, "transport.command");
                assert!(message.contains("fake-mcp"));
            }
            other => panic!("expected Error::Validation, got {other:?}"),
        }
    }

    #[test]
    fn resolve_stdio_rejects_workspace_binary() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("workspace");
        std::fs::create_dir_all(&root).unwrap();

        let ws_bin = root.join("bin");
        let ws_exec = ws_bin.join("fake-mcp");
        write_executable(&ws_exec);

        let ws_search = std::env::join_paths([&ws_bin]).unwrap();
        let err = resolve_stdio_command("fake-mcp", &root, Some(&ws_search)).unwrap_err();
        match err {
            Error::Security { message } => {
                assert_eq!(
                    message,
                    "stdio command is inside the agent-writable workspace"
                );
            }
            other => panic!("expected Error::Security, got {other:?}"),
        }

        let orrery_bin = root.join(".orrery").join("bin");
        let orrery_exec = orrery_bin.join("fake-mcp");
        write_executable(&orrery_exec);

        let orrery_search = std::env::join_paths([&orrery_bin]).unwrap();
        let ok_path = resolve_stdio_command("fake-mcp", &root, Some(&orrery_search)).unwrap();
        assert_eq!(ok_path, std::fs::canonicalize(&orrery_exec).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn resolve_stdio_absolute_requires_executable() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("workspace");
        std::fs::create_dir_all(&root).unwrap();

        let ext_bin = tmp.path().join("ext").join("fake-mcp");
        std::fs::create_dir_all(ext_bin.parent().unwrap()).unwrap();
        std::fs::write(&ext_bin, b"#!/bin/sh\n").unwrap();
        let mut perms = std::fs::metadata(&ext_bin).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&ext_bin, perms).unwrap();

        let cmd_str = ext_bin.to_str().unwrap();
        let err = resolve_stdio_command(cmd_str, &root, None).unwrap_err();
        match err {
            Error::Validation { field, .. } => assert_eq!(field, "transport.command"),
            other => panic!("expected Error::Validation, got {other:?}"),
        }

        let mut exec_perms = std::fs::metadata(&ext_bin).unwrap().permissions();
        exec_perms.set_mode(0o755);
        std::fs::set_permissions(&ext_bin, exec_perms).unwrap();

        let resolved = resolve_stdio_command(cmd_str, &root, None).unwrap();
        assert_eq!(resolved, std::fs::canonicalize(&ext_bin).unwrap());
    }

    #[test]
    fn resolve_stdio_relative_with_separator_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let err = resolve_stdio_command("bin/fake-mcp", tmp.path(), None).unwrap_err();
        match err {
            Error::Validation { field, .. } => assert_eq!(field, "transport.command"),
            other => panic!("expected Error::Validation, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn stderr_forwarder_emits_redacted_notices() {
        let redactor = Arc::new(SecretRedactor::new());
        redactor
            .register(&SecretValue::new("hunter2-secret-value"))
            .unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let hub = Arc::new(
            LiveStreamHub::new(
                tmp.path().join("logs"),
                LiveStreamConfig::default(),
                redactor,
            )
            .unwrap(),
        );
        let input = &b"first line\n\n   \nleaks hunter2-secret-value\n"[..];
        let handle = spawn_stderr_forwarder("fs".to_string(), input, Arc::clone(&hub));
        handle.await.unwrap();

        let events = hub.recent(10).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, StreamEventKind::SystemNotice);
        assert_eq!(events[0].text, "[mcp:fs] first line");
        assert_eq!(events[1].kind, StreamEventKind::SystemNotice);
        assert_eq!(events[1].text, "[mcp:fs] leaks [REDACTED]");
    }

    #[test]
    fn attach_stream_first_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let hub_a = Arc::new(
            LiveStreamHub::new(
                tmp.path().join("logs_a"),
                LiveStreamConfig::default(),
                Arc::new(SecretRedactor::new()),
            )
            .unwrap(),
        );
        let hub_b = Arc::new(
            LiveStreamHub::new(
                tmp.path().join("logs_b"),
                LiveStreamConfig::default(),
                Arc::new(SecretRedactor::new()),
            )
            .unwrap(),
        );
        let connector = RmcpConnector {
            secrets: Arc::new(InMemorySecretStore::new()),
            project_root: tmp.path().to_path_buf(),
            search_path: None,
            stream: std::sync::OnceLock::new(),
        };
        connector.attach_stream(Arc::clone(&hub_a));
        connector.attach_stream(Arc::clone(&hub_b));
        assert!(Arc::ptr_eq(connector.stream.get().unwrap(), &hub_a));
    }

    #[tokio::test]
    async fn catalog_returns_unfiltered_tools() {
        let mut gatekeeper = McpGatekeeper::new();
        let transport = Arc::new(
            MockMcpTransport::new("fs")
                .with_tool("read_file", json!({}))
                .with_tool("read.file", json!({})),
        );
        gatekeeper.register("fs", transport, Vec::new());
        gatekeeper.refresh_catalog().await.unwrap();

        let names: Vec<&str> = gatekeeper
            .catalog()
            .iter()
            .map(|t| t.name.as_str())
            .collect();
        assert_eq!(names, vec!["read_file", "read.file"]);
    }
}
