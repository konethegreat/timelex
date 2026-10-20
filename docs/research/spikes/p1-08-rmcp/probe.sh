#!/bin/bash
# Written by Kone Tshivhinda & Claude
# Raw spec-conformant probes of the rmcp 3.6.0 spike (MCP 2026-07-28). Usage: probe.sh PORT
P=${1:-8090}
U=http://127.0.0.1:$P/mcp
META='"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{"name":"probe","version":"0"},"io.modelcontextprotocol/clientCapabilities":{}}'
METAT='"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{"name":"probe","version":"0"},"io.modelcontextprotocol/clientCapabilities":{"extensions":{"io.modelcontextprotocol/tasks":{}}}}'
H=(-H 'content-type: application/json' -H 'accept: application/json, text/event-stream')

post() { # name, extra-headers..., body last
  local name="$1"; shift
  local body="${@: -1}"; local hdrs=("${@:1:$#-1}")
  echo "=== $name"
  curl -s -i -m 10 "${H[@]}" "${hdrs[@]}" -d "$body" "$U" | tr -d '\r' | grep -v -i -E '^(date|vary|content-length|transfer-encoding):' | head -30
  echo
}

post "1 server/discover" -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: server/discover' \
  "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"server/discover\",\"params\":{$META}}"

post "2 tools/list" -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: tools/list' \
  "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{$META}}"

post "3 tools/call echo" -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: tools/call' -H 'Mcp-Name: echo' \
  "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"echo\",\"arguments\":{\"text\":\"hello\"},$META}}"

post "4 tools/call slow_job WITH tasks capability" -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: tools/call' -H 'Mcp-Name: slow_job' \
  "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"slow_job\",\"arguments\":{},$METAT}}"

post "5 tools/call slow_job WITHOUT tasks capability (expect -32021)" -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: tools/call' -H 'Mcp-Name: slow_job' \
  "{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"tools/call\",\"params\":{\"name\":\"slow_job\",\"arguments\":{},$META}}"

# poll a task: create then get immediately, then after 3s
TID=$(curl -s -m 10 "${H[@]}" -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: tools/call' -H 'Mcp-Name: slow_job' \
  -d "{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"tools/call\",\"params\":{\"name\":\"slow_job\",\"arguments\":{},$METAT}}" "$U" | grep -o '"taskId":"[^"]*"' | head -1 | cut -d'"' -f4)
echo "=== 6 task id: $TID"
GET="{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tasks/get\",\"params\":{\"taskId\":\"$TID\",$METAT}}"
post "6a tasks/get (immediately)" -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: tasks/get' -H "Mcp-Name: $TID" "$GET"
sleep 3
post "6b tasks/get (after 3s; new request, no connection state)" -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: tasks/get' -H "Mcp-Name: $TID" "$GET"

post "7 missing _meta (spec: 400 + -32602)" -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: tools/list' \
  '{"jsonrpc":"2.0","id":8,"method":"tools/list","params":{}}'

post "8 Mcp-Name header mismatch (spec: 400 + -32020)" -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: tools/call' -H 'Mcp-Name: slow_job' \
  "{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"tools/call\",\"params\":{\"name\":\"echo\",\"arguments\":{\"text\":\"x\"},$META}}"

post "9 Mcp-Method header mismatch (spec: 400 + -32020)" -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: tools/list' \
  "{\"jsonrpc\":\"2.0\",\"id\":10,\"method\":\"tools/call\",\"params\":{\"name\":\"echo\",\"arguments\":{\"text\":\"x\"},$META}}"

post "10 missing Mcp-Method header entirely (spec: 400 + -32020)" -H 'MCP-Protocol-Version: 2026-07-28' \
  "{\"jsonrpc\":\"2.0\",\"id\":11,\"method\":\"tools/list\",\"params\":{$META}}"

post "11 unsupported protocol version (spec: 400 + -32022 with data.supported)" -H 'MCP-Protocol-Version: 1900-01-01' -H 'Mcp-Method: tools/list' \
  '{"jsonrpc":"2.0","id":12,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"1900-01-01","io.modelcontextprotocol/clientCapabilities":{}}}}'

post "12 header says 2026-07-28 but body _meta says 2025-11-25 (spec: 400 + -32020)" -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: tools/list' \
  '{"jsonrpc":"2.0","id":13,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2025-11-25","io.modelcontextprotocol/clientCapabilities":{}}}}'

echo "=== 13 GET (spec: 405)"; curl -s -i -m 5 -H 'accept: text/event-stream' "$U" | tr -d '\r' | head -3; echo
echo "=== 14 DELETE (spec: 405)"; curl -s -i -m 5 -X DELETE "$U" | tr -d '\r' | head -3; echo

post "15 Mcp-Session-Id on a modern request (spec: ignore, never echo)" -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: tools/list' -H 'Mcp-Session-Id: abc' \
  "{\"jsonrpc\":\"2.0\",\"id\":14,\"method\":\"tools/list\",\"params\":{$META}}"

echo "=== 16 Origin: https://evil.example (spec: 403 if invalid; rmcp default has no origin allow-list)"
curl -s -i -m 5 "${H[@]}" -H 'Origin: https://evil.example' -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: tools/list' \
  -d "{\"jsonrpc\":\"2.0\",\"id\":15,\"method\":\"tools/list\",\"params\":{$META}}" "$U" | tr -d '\r' | head -3; echo

echo "=== 17 Host: timelex.example (rmcp default allows only loopback Host)"
curl -s -i -m 5 "${H[@]}" -H 'Host: timelex.example' -H 'MCP-Protocol-Version: 2026-07-28' -H 'Mcp-Method: tools/list' \
  -d "{\"jsonrpc\":\"2.0\",\"id\":16,\"method\":\"tools/list\",\"params\":{$META}}" "$U" | tr -d '\r' | head -3; echo

post "18 legacy initialize (2025-11-25) against a server with legacy_session_mode=false" \
  '{"jsonrpc":"2.0","id":17,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"legacy","version":"0"}}}'
