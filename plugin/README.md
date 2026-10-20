# TimeLex Claude plugin

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

**Placeholder.** The gateway does not serve `/mcp` yet (task P1-08, then P4-08). This folder fixes the plugin shape so it can be filled in without changing the repository layout.

When live, the plugin lets a user's own Claude call TimeLex tools (see [contracts/mcp/tools.md](../contracts/mcp/tools.md)) against **their** tenant, authenticated with OAuth 2.1 or an API key, with mutating tools requiring an idempotency key and approvals handled through the MCP multi-round-trip flow. Tools that move money or send to clients are not exposed by default.
