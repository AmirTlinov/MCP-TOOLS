| Case | Status | Duration (ms) | Notes |
| --- | --- | --- | --- |
| list_tools_sse | ✅ | 1 | {"tool_count":4,"url":"http://127.0.0.1:9100/sse"} |
| list_tools_http | ✅ | 1 | {"tool_count":4,"url":"http://127.0.0.1:9101/mcp"} |
| describe_help_sse | ✅ | 0 | {"tool":{"description":"Return a list of mock tools and usage hints.","inputSchema":{"$schema":"http://json-schema.org/draft-07/schema#","title":"MockHelpArgs","type":"object"},"name":"help"}} |
| describe_help_http | ✅ | 0 | {"tool":{"description":"Return a list of mock tools and usage hints.","inputSchema":{"$schema":"http://json-schema.org/draft-07/schema#","title":"MockHelpArgs","type":"object"},"name":"help"}} |
| call_help_sse | ✅ | 1 | {"content":[{"text":"{\"tools\":[{\"name\":\"help\",\"usage\":\"help\"},{\"name\":\"echo\",\"usage\":\"echo text=\\\"hello\\\"\"},{\"name\":\"add\",\"usage\":\"add values=[1,2,3]\"}]}","type":"text"}],"isError":false,"structuredContent":{"tools":[{"name":"help","usage":"help"},{"name":"echo","usage":"echo text=\"hello\""},{"name":"add","usage":"add values=[1,2,3]"}]}} |
| call_help_http | ✅ | 1 | {"content":[{"text":"{\"tools\":[{\"name\":\"help\",\"usage\":\"help\"},{\"name\":\"echo\",\"usage\":\"echo text=\\\"hello\\\"\"},{\"name\":\"add\",\"usage\":\"add values=[1,2,3]\"}]}","type":"text"}],"isError":false,"structuredContent":{"tools":[{"name":"help","usage":"help"},{"name":"echo","usage":"echo text=\"hello\""},{"name":"add","usage":"add values=[1,2,3]"}]}} |
| probe_sse | ✅ | 0 | {"error":null,"latency_ms":0,"url":"http://127.0.0.1:9100/sse"} |
| probe_http | ✅ | 0 | {"error":null,"latency_ms":0,"url":"http://127.0.0.1:9101/mcp"} |
| negative_missing_command | ✅ | 0 | {"expected_error":true,"response":{"error":"missing command for stdio","latency_ms":null,"ok":false,"server_name":null,"transport":"stdio","version":null}} |

Pass rate: 100.00% (threshold 95%)