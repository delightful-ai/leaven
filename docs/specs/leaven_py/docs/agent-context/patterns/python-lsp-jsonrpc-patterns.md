# python-lsp-jsonrpc: Minimal JSON-RPC Patterns for ACP Workers

120 KB reference implementation from python-lsp (Palantir). Pure JSON-RPC over stdio with Content-Length framing.
Used as minimal model for Python-side `lv.serve_stage(...)` worker behavior and ACP stdio transport.

## 1. Read First (Entry Points)

| File | Lines | Role |
|------|-------|------|
| `endpoint.py` | 18-262 | Endpoint dispatch: bifurcates req/resp/notify; async via ThreadPoolExecutor |
| `streams.py` | 15-112 | Stream framing: Content-Length reader/writer with thread-safe buffering |
| `dispatchers.py` | 11-38 | Method dispatch: converts camelCase + slashes → m_snake_case handlers |
| `exceptions.py` | 7-113 | Error contracts: JSON-RPC error codes + ` from_dict`/`to_dict` round-trip |

## 2. The Minimal Viable JSON-RPC Shape

**Architecture:** One `Endpoint` object owns dispatch, future tracking, and callback routing.

```
Endpoint(dispatcher, consumer)
├─ dispatcher: dict[method_name → callable(params) → result | Future | callable]
├─ _consumer: function that serializes & writes each message
├─ _client_request_futures: tracks pending requests to client
├─ _server_request_futures: tracks pending responses from client
└─ _executor_service: ThreadPoolExecutor for async work

Wire:
  client → [JsonRpcStreamReader] → endpoint.consume(dict) → dispatch
  dispatch → endpoint._consumer → [JsonRpcStreamWriter] → client
```

**Key decision at endpoint.py:190-217**: handler can return:
- Synchronous value → immediate response (endpoint.py:208-217)
- Callable → async execution via executor (endpoint.py:199-207)
- `futures.Future` → already-pending work (endpoint.py:204-207)

**Bifurcation at endpoint.py:98-131**: `consume()` splits on presence of `id`:
- `id` absent + `method` present → notification (endpoint.py:108-149)
- `id` present + `method` absent → response to an earlier request (endpoint.py:111-113, 247-261)
- `id` and `method` both present → incoming request (endpoint.py:114-131)

## 3. The Framing Choice: Content-Length vs Line-Delimited

**What pylsp_jsonrpc does (streams.py:45-80)**:
- Reads `Content-Length: <N>` header line (streams.py:51, 56-79)
- Blindly consumes all header lines until blank line (streams.py:59-60)
- Reads exactly `N` bytes of body (streams.py:66)
- Writes headers + `\r\n\r\n` + JSON body (streams.py:102-108)

LSP heritage: inherited from Language Server Protocol (Microsoft), which borrowed from HTTP. Framing overhead ~2×.

**Leaven/ACP choice (per leaven-acp stdio contract)**:
- Line-delimited JSON (`\n` terminator)
- No headers, no Content-Length computation
- Simpler stdin/stdout parsing, smaller overhead

**Verdict**: pylsp_jsonrpc choice works but carries "accidental complexity" (matklad). If Python worker sees `Content-Length`, it came from LSP lineage, not minimal JSON-RPC. leaven-acp contracts for line-delimited; worker must adapt.

## 4. Patterns Worth Stealing for leaven-acp Client (Python Side)

### Pattern: Dual-direction futures tracking (endpoint.py:36-37, 82-88, 247-261)

Two independent future maps allow bidirectional request/response pairing:
```python
self._client_request_futures = {}  # endpoint sent request; awaiting response
self._server_request_futures = {}  # endpoint received request; response pending
```

Allows server to send request (e.g., ask engine for case data) while handling client request in same thread. Critical for worker that must call back to engine during evaluation.

**File:line**: endpoint.py:36–37 (dict declarations), 82–88 (client request tracking), 249–261 (response routing).

### Pattern: Callable-as-async-work marker (endpoint.py:199-207)

Handler function can signal "this is async" by returning a `callable` (zero-arg function):
```python
result = handler(params)
if callable(result):
    # User said "do this later" — we own execution
    future = executor.submit(result)  # No args; callable is closure
```

No explicit `async`/`await` syntax needed from user. Closure captures context. Handler signature is flat: `def m_foo(params)`.

**File:line**: endpoint.py:199–207 (async request), 145–154 (async notification).

### Pattern: Error serialization round-trip (exceptions.py:15-44)

Errors marshal through `to_dict()` / `from_dict()`:
```python
exc.to_dict() → {'code': -32601, 'message': 'Method Not Found', 'data': ...}
JsonRpcException.from_dict(dict) → correct exception subclass
```

Codes are negative integers per JSON-RPC spec. `from_dict` factory dispatches on code. Survives wire and unpacks to correct exception type.

**File:line**: exceptions.py:15–44 (to_dict/from_dict), 57–63 (MethodNotFound example).

### Pattern: Method name mangling (dispatchers.py:18-38)

Caller sends method string (e.g., `"textDocument/didOpen"`, `"$/cancelRequest"`).
Dispatcher converts to Python handler name:
```python
"textDocument/didOpen" → m_text_document__did_open
"$/cancelRequest"       → m_cancel_request
```

Handles camelCase → snake_case, `/` → `__`, `$` → drop.
User writes `def m_text_document__did_open(self, params)`.

**File:line**: dispatchers.py:31–37 (naming conversion).

## 5. Patterns We Should NOT Copy

### Anti-pattern: Broad try/except with logging-only failure (endpoint.py:145-149, 239-241)

```python
try:
    handler_result = handler(params)
except Exception:  # broad-except
    log.exception("Failed to handle notification ...")
    return  # Silent swallow
```

For notifications (fire-and-forget), this is acceptable. For requests, silent swallow violates JSON-RPC: client waits forever.

**Why not for leaven-acp**: Worker must validate params, enforce type contracts, and signal schema mismatches. Python worker side needs stricter error propagation.

**File:line**: endpoint.py:145–149 (notification handling), 125–131 (request handling does better).

### Anti-pattern: Thread-local future state (endpoint.py:38)

```python
self._executor_service = futures.ThreadPoolExecutor(max_workers=5)
```

Hard-coded pool. No context about how many stages will run or CPU budget. Fine for LSP (typically one editor session), not for heavy GEPA eval that might spawn 100 workers.

**Why not for leaven-acp**: Worker pool size should be negotiated with engine or configured per stage.

**File:line**: endpoint.py:38.

## 6. What Would NOT Survive Adversarial Review

### Thread safety gap in streams.py:28

```python
while not self._rfile.closed:
    try:
        request_str = self._read_message()
    except ValueError:
        if self._rfile.closed:
            return
```

Condition `self._rfile.closed` is checked twice (startup + exception) without lock. Race: file closes between check and read. Unlikely in single-reader case (LSP), but not hardened.

**For leaven-acp**: ACP contract already owns process lifecycle; reader shouldn't check closure state itself.

**File:line**: streams.py:28–34.

### Missing request ID validation

`endpoint.consume()` accepts any `id` value without validating uniqueness or format. If client sends duplicate IDs, futures map silently overwrites.

```python
# No validation that msg_id is unique or well-formed
request_future = futures.Future()
self._server_request_futures[msg_id] = request_future  # Can overwrite
```

**For leaven-acp**: Engine will send UUIDs. Validation should happen at seam boundary, not deep in dispatch.

**File:line**: endpoint.py:82–88.

## 7. Surprises + Open Questions

**Surprise**: MethodDispatcher uses `__getitem__` to intercept method lookup (dispatchers.py:18-28). Not obvious that `dispatcher[method]` is actually `dispatcher.__getitem__(method)`. User sees `dispatcher = LanguageServer()` and doesn't realize it's callable.

**Why it works**: Python dict protocol. Caller does `self._dispatcher[method]` at endpoint.py:140, 193.

**For leaven-acp**: Could use a Dispatcher ABC instead, making contract explicit. Current code is idiomatic but requires reading dispatchers.py to understand the shape.

**Question**: Why two separate future maps? Why not one `BTreeMap[(msg_id, direction)] → Future`?

**Answer**: Code clarity; server/client are conceptually different pairings. But adds bookkeeping complexity. Each request must unwind both if it cancels.

**For leaven-acp**: ACP starts clean. Could use a single indexed future store if direction is tagged on the message itself.

**Question**: Does `listen()` block forever or can it timeout?

**Answer**: Blocks on `rfile.readline()` forever. No timeout. If worker hangs, engine subprocess waits forever unless parent process enforces timeout.

**For leaven-acp**: ACP contract should own timeout policy at session level, not stream level.

**File:line**: streams.py:22–43 (listen signature).

---

**Summary (150w)**: 
Two core patterns to steal:
1. **Dual futures maps** (endpoint.py:36–37): bidirectional request/response pairing. Critical when worker calls back to engine during same execution.
2. **Callable-as-async marker** (endpoint.py:199–207): handler returns `callable` to say "do this later." No explicit async/await syntax.

Framing choice:
pylsp_jsonrpc uses LSP's Content-Length framing (headers + body). leaven-acp chose line-delimited (simpler, smaller). Conversion layer needed at seam.

**Smallest vendored entry**: `/Users/darin/src/personal/leaven/docs/specs/leaven_py/repos/python-lsp-jsonrpc/pylsp_jsonrpc/endpoint.py`
