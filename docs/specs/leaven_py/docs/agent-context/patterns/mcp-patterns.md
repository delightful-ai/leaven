# MCP Python SDK Pattern Observations

Vendored reference for Leaven's stdio JSON-RPC wire handling, FastMCP decorators, and failure modes.
MCP is **not** protocol compliance (Leaven owns ACP profile), but patterns here inform:
- `src/leaven/decorators.py` (`serve_stage` standalone-worker shape)
- Future `leaven-acp` Rust crate (mirror Windows CRLF stdio issue)
- `src/leaven/x/dspy/lm.py` (transport-aware adapter)

**Vendored path:** `repos/mcp-python-sdk/` (May 2024 snapshot)

---

## 1. What to Read First

| File | Why |
|------|-----|
| `src/mcp/server/stdio.py` | Line-delimited JSON-RPC framing over stdin/stdout; TextIOWrapper platform workaround |
| `src/mcp/types/jsonrpc.py` | JSON-RPC 2.0 message models + error codes (e.g., `URL_ELICITATION_REQUIRED = -32042`) |
| `src/mcp/server/session.py` | ServerSession state machine, capability negotiation, request/response lifecycle |
| `src/mcp/server/mcpserver/server.py` | FastMCP decorator registry (l.512–580 for `@server.tool()` pattern) |
| `src/mcp/server/mcpserver/tools/base.py` | Tool introspection: `Tool.from_function()` + async/context injection |

---

## 2. Stdio JSON-RPC Framing Pattern

**Transport:** Line-delimited JSON over stdin/stdout (one message = one line).

**Key file:** `src/mcp/server/stdio.py:1–78`

```python
# Lines 42–44: TextIOWrapper re-wrapping ensures UTF-8 on all platforms
stdin = anyio.wrap_file(TextIOWrapper(sys.stdin.buffer, encoding="utf-8", errors="replace"))
stdout = anyio.wrap_file(TextIOWrapper(sys.stdout.buffer, encoding="utf-8"))

# Line 54: Validation via Pydantic adapter
message = types.jsonrpc_message_adapter.validate_json(line, by_name=False)

# Line 69: One message → one line + flush
await stdout.write(json + "\n")
await stdout.flush()
```

**Design choices:**
- **Line-delimited, not Content-Length:** simpler for stdio, but no message framing for binary-safe transport.
- **TextIOWrapper with `errors="replace"`:** handles malformed UTF-8 on stdin gracefully; stdout strict for symmetry.
- **Async task group for stdin reader + stdout writer:** bidirectional concurrency without blocking.

**Leaven implication:** `serve_stage` workers over stdio should mirror this framing exactly.

---

## 3. FastMCP Decorator Framework

**Pattern:** Decoration → manager registry → dispatcher lookup.

**Key files:**
- `src/mcp/server/mcpserver/server.py:512–580` — decorator definition
- `src/mcp/server/mcpserver/tools/base.py:44–89` — `Tool.from_function()` introspection
- `src/mcp/server/mcpserver/tools/tool_manager.py:18–86` — manager + registry

**Example flow:**

```python
# src/mcp/server/mcpserver/server.py:512–580
@server.tool()
def my_tool(x: int) -> str:
    return str(x)

# Decorator calls add_tool() → ToolManager.add_tool() → Tool.from_function()
# Tool.from_function() inspects function signature, extracts JSON schema, detects async

# At dispatch time (src/mcp/server/mcpserver/server.py:307–316):
async def _handle_call_tool(self, ctx, params: CallToolRequestParams):
    context = Context(request_context=ctx, mcp_server=self)
    result = await self.call_tool(params.name, params.arguments or {}, context)
```

**Introspection details** (`src/mcp/server/mcpserver/tools/base.py:70–89`):
- Async detection via `is_async_callable(fn)`
- Context parameter detection via `find_context_parameter(fn)`
- Pydantic schema extraction from function signature

**Leaven implication:** `serve_stage` decorator should use similar `Tool.from_function()` style introspection for registration.

---

## 4. Session Lifecycle

**State machine:** `InitializationState` enum (lines 55–58 of `src/mcp/server/session.py`)

```python
class InitializationState(Enum):
    NotInitialized = 1
    Initializing = 2
    Initialized = 3
```

**Handshake sequence** (`src/mcp/server/session.py:165–189`):

1. Client sends `InitializeRequest` → `_received_request()` matches on `InitializeRequest`.
2. Server sets `_initialization_state = Initializing`, captures `_client_params`.
3. Server responds with `InitializeResult` (protocol version, capabilities, server info).
4. Server sets `_initialization_state = Initialized`.
5. Client sends `InitializedNotification` → server confirms (l.201–202).

**Key design:**
- **Strict initialization guard** (l.194–195, 204–205): all non-ping requests rejected until `Initialized`.
- **Capability negotiation via `client_params`** (l.122–159): `check_client_capability()` validates required client features.
- **Request/response correlation:** via JSON-RPC `id` field (handled by base `BaseSession`).

**Cancellation & shutdown:** Not visible in this excerpt, but handled at `BaseSession` level (parent class).

---

## 5. Known Failure Modes Worth Mirroring

### 5.1 Windows CRLF Stdio Corruption (Issue #552)

**The bug:** On Windows, `sys.stdin` / `sys.stdout` are opened in text mode with CRLF line endings.
If MCP messages are passed through without re-wrapping, JSON lines become `{...}\r\n` instead of `{...}\n`,
breaking the framing assumption.

**Evidence:**
- Test file: `tests/issues/test_552_windows_hang.py:1–64`
- Regression: `test_windows_stdio_client_with_session()` verifies the exact hang scenario from the bug report.

**The fix** (`src/mcp/server/stdio.py:40–44`):

```python
# Re-wrap the binary stream explicitly with UTF-8, not platform default
stdin = anyio.wrap_file(TextIOWrapper(sys.stdin.buffer, encoding="utf-8", errors="replace"))
stdout = anyio.wrap_file(TextIOWrapper(sys.stdout.buffer, encoding="utf-8"))
```

**Leaven action:** The Rust `leaven-acp` crate must do the same at the OS level:
- Detect `cfg!(target_os = "windows")` and force newline-agnostic frame parsing (e.g., trim `\r\n` or use length-prefixed framing).
- Or, normalize stdio to UTF-8 without platform CRLF injection at process startup.

### 5.2 Cleanup Timeout (Issue #1027)

**The issue:** On process termination, lifespan cleanup code (after `yield`) was unreachable.

**The fix:** Close stdin first, giving the server's event loop a graceful exit signal before killing the process.

**Evidence:** `tests/issues/test_1027_win_unreachable_cleanup.py:23–44` verifies cleanup marker files are written.

**Leaven action:** Ensure `serve_stage` workers close stdin on graceful shutdown before force-terminating.

---

## 6. Idioms Worth Stealing for `serve_stage`

1. **Decorator + introspection:** Use `@serve_stage.tool()` with automatic schema extraction from function signature.
2. **Manager registry pattern:** Tools/resources in a dict keyed by name; lookup on dispatch.
3. **Context injection via parameter type:** Auto-detect `context: Context` param and inject session state.
4. **Async task group for bidirectional I/O:** Concurrent stdin reader + stdout writer, shared message streams.
5. **Pydantic for JSON schema:** Auto-generate tool input schemas from function arguments.
6. **Capability negotiation:** Store `client_params` at initialization, let handlers check features before proceeding.

---

## 7. Where Leaven Correctly Diverges

1. **Protocol surface:** Leaven owns the ACP profile (locked at `public-seam-v1/profiles/leaven_acp_profile_v1_v0.3.md`), not MCP.
   - MCP's `Tool`, `Resource`, `Prompt`, `Completion` concepts map to Leaven ACP concepts but are **not** interchangeable.
   - Leaven's stage/payload/evidence model is orthogonal; do not reuse MCP type names.

2. **Decorator ergonomics:** MCP's `@server.tool()` returns the function unchanged. Leaven may wrap it differently (e.g., for resumability, evidence capture).

3. **Transport:** MCP supports stdio, SSE, HTTP. Leaven starts with stdio but owns its own transport layer. Do not assume MCP's transports work for ACP.

4. **Error codes:** MCP uses JSON-RPC error codes (-32000 range). Leaven ACP error codes are separate. Use ACP's error contract.

---

## 8. Surprises (Load-Bearing Non-Obvious Decisions)

1. **TextIOWrapper `errors="replace"`:** Silently replaces invalid UTF-8 bytes on stdin, not raising. This is **intentional**—robust to malformed input from untrusted clients. Stdout is strict (no `errors=` arg), enforcing the server produces valid JSON.

2. **Tool name validation warning:** `ToolManager.add_tool()` logs a duplicate-name warning but does **not** error or reject. The second registration silently replaces the first. This is a footgun if not careful.

3. **`find_context_parameter()` magic:** The decorator introspects the function to detect a `context: Context` parameter by type annotation, then injects it automatically. The parameter name can be anything (`ctx`, `context`, etc.). This is implicit and not obvious from the decorator signature.

4. **Stateless mode:** `ServerSession` has a `stateless=False` parameter. In stateless mode, initialization is skipped. Not documented in the session.py docstring—only in the constructor. This is a landmine for stateless HTTP deployments.

---

## 9. Open Questions

1. **Request/response correlation on slow clients:** How does MCP handle client-side timeouts vs. server response latency? Does the SDK have per-request timeouts?

2. **Backpressure on tool/resource registration:** If a server registers 10k tools, what's the memory footprint? Is there a manager-level limit or lazy-loading pattern?

3. **Resource URI template matching:** How does MCP's `ResourceTemplate` URI template matching work (e.g., wildcards, regex)? Not visible in `mcpserver/resources/` subdirectory—likely in lowlevel.

4. **Sampling request context:** The `create_message()` overloads have `include_context` parameter. What's the semantic difference between sampling with and without context, and how does Leaven's evidence model map?

5. **Lifespan in stateless HTTP:** How does the `lifespan` async context manager work in stateless/SSE mode? Does each request get a new lifespan, or is it global? Not clear from server.py lines 111–127.

---

## 10. Citation Map

| Pattern | File:Lines |
|---------|-----------|
| Stdio framing | `src/mcp/server/stdio.py:32–78` |
| JSON-RPC models | `src/mcp/types/jsonrpc.py:13–84` |
| Session state machine | `src/mcp/server/session.py:55–159` |
| Handshake sequence | `src/mcp/server/session.py:165–189` |
| Decorator definition | `src/mcp/server/mcpserver/server.py:512–580` |
| Tool introspection | `src/mcp/server/mcpserver/tools/base.py:44–89` |
| Manager registry | `src/mcp/server/mcpserver/tools/tool_manager.py:18–86` |
| Call dispatch | `src/mcp/server/mcpserver/server.py:307–316` |
| Windows CRLF fix | `src/mcp/server/stdio.py:40–44` |
| Issue #552 test | `tests/issues/test_552_windows_hang.py:13–64` |
| Issue #1027 test | `tests/issues/test_1027_win_unreachable_cleanup.py:23–44` |
| Context injection | `src/mcp/server/mcpserver/tools/base.py:67–68` |
| Capability check | `src/mcp/server/session.py:122–159` |

