# P1-08 spike: rmcp 3.6.0 against MCP 2026-07-28

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude · 20 Oct 2026_

Throwaway code that answers one question: **does the official Rust SDK serve the 2026-07-28 spec statelessly over Streamable HTTP, with Tasks?** Findings: [docs/research/08-mcp-rust-sdk-spike.md](../../08-mcp-rust-sdk-spike.md). This is not product code and is not in the Cargo workspace.

```
cargo run                 # lax server on 127.0.0.1:8090
cargo run -- --strict     # strict server on 127.0.0.1:8091
bash probe.sh 8090        # 18 raw-HTTP probes with spec expectations in each label
```

`probe.sh` needs `bash` and `curl` (Git Bash on Windows is fine). `results.txt` is the output from 20 Oct 2026.

Known shortcut in the spike: `tasks/get` rebuilds the `Task` each call, so its `createdAt` changes between polls. A real service stores the task row in Postgres. That is a spike bug, not an SDK finding.
