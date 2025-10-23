use assert_cmd::cargo::cargo_bin;

pub fn mcp_multi_tool_path() -> String {
    cargo_bin("mcp-multi-tool").display().to_string()
}

pub fn mock_server_path() -> String {
    cargo_bin("mock_mcp_server").display().to_string()
}
