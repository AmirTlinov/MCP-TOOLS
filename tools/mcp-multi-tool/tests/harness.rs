use assert_cmd::cargo::cargo_bin;

pub struct MockMcpCommand;

impl MockMcpCommand {
    pub fn stdio_path() -> String {
        cargo_bin("mcp-multi-tool").display().to_string()
    }
}
