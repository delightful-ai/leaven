# 03 — Result and Receipt Spec v0.3

`final_revision` is always present.

For read-only plans, `final_revision == base_revision`.

Plan-level replayability is only a roll-up summary.

Values carry replayability.

Assessment batch receipts carry per-assessment replayability.

Graph rows are typed.

Case records are typed.

Workspace handles are typed.

Workspace files are typed.

Workspace diffs are typed.

Workspace listings are typed.

LM responses are typed.

Agent sessions are typed.

Sandbox exec results are typed.

Write result IDs are typed by write kind.

Query receipts carry op hash and result hash.

Call receipts carry request hash and result hash.

Write receipts carry request hash and result hash.

Receipts carry `started_at` and `completed_at`.

Receipts can carry typed errors.

Plan errors use a closed enum.

Plan errors may reference the related operation receipt.

A failed call that spent money still produces a call receipt and charge receipt.

A failed write that validates partially still produces validation receipts.

A receipt is not log decoration.

A receipt is audit currency.
