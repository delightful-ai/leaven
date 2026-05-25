# Jupyter-Client RPC Patterns: Reference for Leaven Python ACP

**Repo:** `jupyter/jupyter_client@main` (vendored at `docs/specs/leaven_py/repos/jupyter-client/`)  
**Scope:** Client/worker lifecycle, message correlation, session abstraction  
**Audience:** Designers of `lv.serve_stage()` subprocess worker lifecycle and leaven-acp Python side

---

## 1. What to Read First

**Mandatory entry points in order:**

1. `jupyter_client/session.py` (lines 308–650):
   - `msg_header()` (line 308): builds request message envelope with `msg_id` + session identity
   - `Session.send()` (line 760): packs/signs/delimiters message across ZMQ frames
   - `Session.recv()` (line 917): unpacks, verifies HMAC, correlates response to request

2. `jupyter_client/client.py` (lines 74–200):
   - KernelClient trait layout: five channels (shell, iopub, stdin, hb, control)
   - `_async_is_alive()` (line 412): heartbeat-based liveness inference
   - Channel lifecycle: connect, run, stop (lines 320–350)

3. `jupyter_client/manager.py` (lines 102–278):
   - KernelManager lifecycle: `start_kernel()` → `client()` → `shutdown_kernel()`
   - Client factory injection pattern (line 264–277)
   - `ready` future (line 210): pending-state wrapper for async process start

4. `jupyter_client/managerabc.py` (lines 9–58):
   - Abstract interface contract: spawn, supervise, interrupt, signal, cleanup

---

## 2. The Session Abstraction

**What jupyter_client means by "session":** not a user session, but a message *envelope layer* that handles correlation between requests and replies.

- **Session ID** (`Session.session`): UUID string unique per client–kernel pair, stamped on every message header
- **Message ID** (`msg_id`): per-request unique string (line 308: `msg_header()` builds new header with fresh `msg_id`)
- **Parent tracking**: replies carry `parent_header` (line 674) so the client correlates `recv()` to prior `send()`
- **HMAC security** (optional): `Session.key` + digest on every frame if configured (lines 530–560)

**Lifecycle:**
1. Client calls `session.send(msg_type, content, parent=None)` → assigns fresh `msg_id`, stamps session UUID, signs/packs
2. Kernel receives, processes, sends reply with `parent_header = request.header` (so `msg_id` matches)
3. Client calls `session.recv(timeout)` → waits for message with matching `parent_header.msg_id`

**Heartbeats** (lines 45–95 in `channels.py`):
- HBChannel runs as daemon thread, sends empty pingers every `time_to_dead` seconds
- Client calls `_async_is_alive()` (line 412): asks hb_channel if last pinged recently
- No round-trip reply expected; liveness = "got a response in the last N seconds"

---

## 3. The Manager Pattern

**KernelManager** (line 102 in `manager.py`): owns one subprocess kernel + coordinates its lifecycle.

**Lifecycle:**
- `start_kernel(**kw)` (line 428): async wrapper around provisioner.launch(command) → stores `_launch_args` for restart
- `_ready` future (line 108–125): set to pending on start, resolved once kernel accepts heartbeats
- `client()` (line 264): factory method—builds KernelClient configured with connection_file, context, parent=self
- `shutdown_kernel(now=False, restart=False)` (line 522): interrupt → signal SIGTERM → wait → SIGKILL if needed
- `restart_kernel()` (line 563): shutdown + start_kernel using stored `_launch_args`

**Cleanup:**
- `__del__()` (line 232): closes ZMQ context if created locally, deletes connection file
- Optional autorestart (line 226): if kernel dies, restarter hook can trigger restart

**Process provisioning** (line 156): KernelProvisioner abstraction allows local subprocess, SSH, containers, etc.

---

## 4. Portable Patterns for `lv.serve_stage()` / leaven-acp

### Pattern 1: Ready-State Future for Async Spawn (STEAL)

**Where:** `KernelManager._ready` (lines 108–214)  
**Why:** subprocess startup is async but user code may be sync. Solution: return a Future immediately, resolve it once process is ready.

```python
# From manager.py line 210
@property
def ready(self) -> t.Union[CFuture, Future]:
    """A future that resolves when the kernel process has started for the first time"""
    if not self._ready:
        self._ready = _get_future()
    return self._ready
```

**For leaven:** `serve_stage()` should return or expose a Future that resolves once stdio handshake succeeds. Caller can `await` it to know worker is live.

### Pattern 2: Stateless Request/Reply Correlation via Headers (STEAL)

**Where:** `Session.send()` / `Session.recv()` (lines 760–950)  
**Why:** no in-memory correlation map needed; each request/reply pair carries `msg_id` and `parent_header`. Surviving process crashes is free.

```python
# From session.py line 308
def msg_header(msg_id: str, msg_type: str, username: str, session: Session | str) -> dict[str, t.Any]:
    """Create a new message header"""
    return {"msg_id": msg_id, "msg_type": msg_type, "username": username, "session": session, ...}

# From session.py line 674
msg["parent_header"] = {} if parent is None else extract_header(parent)
```

**For leaven:** ACP request/reply envelope should stamp `request_id` at send time, copy it to `parent_request_id` in reply. No in-process registry needed; if worker crashes mid-flight, new worker sees no active request.

### Pattern 3: Liveness via Async Inference + Heartbeat (STEAL with caution)

**Where:** `_async_is_alive()` (line 412), `HBChannel` (channels.py lines 33–200)  
**Why:** no explicit handshake; client infers liveness from heartbeat thread's last successful ping timestamp.

```python
# From client.py line 412
async def _async_is_alive(self) -> bool:
    """Check whether the kernel is alive."""
    if self.shell_channel.is_alive():
        return True
    elif self.hb_channel and self.hb_channel.is_alive():
        return self.hb_channel.is_alive()
    return False
```

**For leaven:** Worker should send periodic heartbeats on a dedicated channel (or stderr), client reads timestamp. Don't rely on main RPC channel silence (it can hang mid-request). Portable across stdio, ZMQ, HTTP.

---

## 5. Patterns We Should NOT Copy

### Anti-pattern 1: ZMQ-Specific Frame Delimiters

**Where:** `DELIM = b"<IDS|MSG>"` (session.py line 173), `Session.send_raw()` (line 880)  
**Why:** ZMQ requires frame boundaries; jupyter bundles message parts with explicit delimiter for security and routing.

```python
# From session.py line 739
parts = [
    self.pack(msg["parent_header"]),
    self.pack(msg["header"]),
    self.pack(msg["metadata"]),
    self.pack(msg["content"]),
]
# ZMQ-specific: frames sent as separate parts
```

**For leaven over stdio:** Use newline- or length-delimited JSON instead. If we do migrate to ZMQ later, we can layer it then.

### Anti-pattern 2: HMAC Framing on Every Message

**Where:** `Session.key` (line 482), signature on frame 0 (line 739)  
**Why:** fine for ZMQ peer-to-peer; overkill for subprocess stdio where process boundary = trust boundary.

**For leaven:** Use TLS or skip it in local-subprocess mode. HMAC-on-every-frame adds framing overhead.

### Anti-pattern 3: Traitlets Configuration Cascade

**Where:** `KernelManager.client_class` (line 138), `@observe` / `_client_class_changed` (line 147)  
**Why:** jupyter shares config across CLI tools. Leaven doesn't need this coupling.

**For leaven:** Use plain dependency injection (factory argument), not trait change observers.

---

## 6. What Would NOT Survive Adversarial Review

### Concern 1: Heartbeat Thread Precision Under Load

**Finding:** HBChannel is a daemon thread (channels.py line 63). If kernel is CPU-bound, heartbeat thread may not get scheduled, false-negating `is_alive()`.

**Implication for leaven:** If worker is truly hung (not just slow), heartbeat-only liveness is insufficient. Pair with a timeout on RPC send: if no reply after N seconds, assume dead.

### Concern 2: Provisioner Abstraction Leakage

**Finding:** KernelProvisioner is swappable (manager.py line 156), but start/shutdown interface doesn't expose all failure modes uniformly. SSH provisioner may silently fail to shut down remote process.

**Implication for leaven:** Document cleanup contract explicitly: "shutdown_stage() does not guarantee process termination, only best-effort SIGTERM." Caller owns verifying process is gone.

### Concern 3: No Explicit Request Acknowledgment

**Finding:** jupyter_client assumes shell_channel delivery is atomic. If client sends request and immediately exits, no guarantee kernel received it.

**Implication for leaven:** Before allow-dropping client connection, wait for worker to echo request_id back, or use two-phase: send request → wait for ack → wait for result.

---

## 7. Surprises + Open Questions

1. **Session identity is UUID, not connection-wide state.** Jupyter regenerates session UUID per KernelManager, not per KernelClient. If two clients connect to the same kernel, they see different `session` values in headers. This is intentional (for security): each client is a separate "entity."

   *Implication:* If leaven needs to identify "the stage worker," don't use session UUID; use stable worker ID or connection handle instead.

2. **KernelManager.client() is not cached.** Each call to `manager.client()` creates a fresh KernelClient with its own ZMQ sockets. Jupyter allows this; leaven may not want the overhead.

   *Pattern:* Cache the client in the manager if you'll call it repeatedly (or add a caching layer above serve_stage).

3. **Shutdown is cooperative, not forced until timeout.** KernelManager sends SIGINT (interrupt), waits `shutdown_wait_time/2`, then SIGTERM, then SIGKILL (manager.py line 522). Worker can ignore SIGINT and delay SIGTERM.

   *For leaven:* Expect unruly workers. Timeout-bounded shutdown is non-negotiable.

4. **"is_alive" has no true definition in jupyter.** It's heartbeat-based if HBChannel is running, else shell-channel–based. If both fail, is_alive returns False even if kernel is responsive. No proactive health check.

   *For leaven:* Explicit liveness contract needed: worker MUST send heartbeats on a dedicated channel, or RPC reply counts as liveness proof.

---

## Summary (200 words)

**Three portable patterns:**

1. **Ready-state Future** (manager.py:210): expose `await worker_ready()` to know subprocess is live, without blocking decorator. Supports both sync and async caller.

2. **Stateless request/reply correlation via headers** (session.py:308, 674): stamp `msg_id` at send, echo as `parent_msg_id` in reply. No in-process registry; survives process restart transparent to RPC layer.

3. **Async liveness inference** (client.py:412): infer health from heartbeat thread timestamps. Pair with RPC send-timeout to detect hung workers.

**One ZMQ anti-pattern:**

- **Frame delimiters + HMAC:** use newline- or length-delimited JSON over stdio, skip HMAC for local subprocess (process boundary = trust).

**One review concern:**

- **Heartbeat precision under load:** daemon thread may not fire reliably if kernel is CPU-bound. Pair with explicit RPC timeout.

**File:** `/Users/darin/src/personal/leaven/docs/specs/leaven_py/docs/agent-context/patterns/jupyter-client-patterns.md`
