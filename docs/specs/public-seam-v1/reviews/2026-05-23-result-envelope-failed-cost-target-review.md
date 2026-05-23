# Public Seam V1 Result Envelope, Failed Cost, and Target Read Review

Scope:

- `ps1.result.typed_envelope`
- `ps1.receipts.failed_costs`
- `ps1.evaluator.target_reads`

Reviewer:

- Read-only adversarial sub-agent Curie, id `019e56f3-959f-7743-890e-d00376f57ee1`.

Review rule:

- The reviewer was instructed not to treat rerunning implementer tests as proof. The review used semantic inspection of the locked specs, matrix rows, implementation, tests, and prior review notes.

Findings:

- `ps1.evaluator.target_reads` must remain pending. The then-current evidence proved representative `case_query.load` Plan IR reads and receipt binding, but not evaluator capability or evaluation-request scope. The reviewer specifically noted that the public seam request/context carried no grant state and the negatives did not deny non-evaluator or out-of-request reads.
- `ps1.result.typed_envelope` should remain pending until the matrix cites a successful plan-run production test. The reviewer found no obvious code blocker once the existing successful execution test is cited, but did not sign off the row as written.
- `ps1.receipts.failed_costs` has no blocking finding for the controlled failed `lm_complete` path. The reviewer found the engine-ledger projection plus public-seam validation sufficient for this row, scoped to the controlled failed LM-cost path. This must not be described as agent/sandbox failure proof, ACP delivery proof, durable persistence proof, or provider integration proof beyond projected public-seam documents.

Resolution:

- `ps1.evaluator.target_reads` remains pending. Follow-up implementation adds capability-authorized `case_query.load` execution and denial tests, but runtime evaluator lowering is still not proven.
- `ps1.result.typed_envelope` remains pending. The matrix now cites the successful public-seam plan execution test as partial evidence.
- `ps1.receipts.failed_costs` may move to proven with the scoped evidence already present in the matrix and with this review recorded.
