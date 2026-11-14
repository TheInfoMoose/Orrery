use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::config::{
    McpServerConfig, RoleConfig, WriteToolSpec, format_mapped_tool_name,
    validate_concrete_tool_ref, validate_safe_relative_path,
};
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
        McpConnector, McpGatekeeper, McpTransport, MockMcpConnector, MockMcpTransport, WriteScope,
        scopes_overlap, tool_result_is_error, tool_result_text,
    };
    use crate::Error;
    use crate::config::{McpServerConfig, McpTransportConfig, RoleConfig, RoleKind, WriteToolSpec};

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
}
