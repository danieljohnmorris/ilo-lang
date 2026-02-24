# Idea 6: MCP Composition Layer

ilo as a thin layer on top of Model Context Protocol. Tools are already MCP servers. ilo adds composition, data flow, and error handling.

- Tools referenced by `mcp://` URIs
- MCP already defines tool discovery, typed schemas, and invocation
- ilo adds: sequencing (chain calls with data flow), error handling (`err` on each step), verification (all URIs resolved before execution, all types checked)
- `compensate` for rollback on multi-step failures
- The agent discovers tools via MCP, composes them via ilo
