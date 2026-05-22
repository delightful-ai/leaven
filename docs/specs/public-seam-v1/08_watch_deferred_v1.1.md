# 08 — Watch Deferred to v1.1

`watch.v1` is not locked in v1.

The current use cases can use finite diff queries through `consistency.since_revision`.

A real watch protocol requires delivery semantics.

A real watch protocol requires backpressure.

A real watch protocol requires cursor and ack semantics.

A real watch protocol requires lifetime semantics.

A real watch protocol requires cancellation and heartbeat semantics.

Shipping a thin watch schema would freeze the wrong abstraction.

The v0.3 bundle includes only a deferred marker schema.
