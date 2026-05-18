# GEPA Parity Working Ledger

Status: active.
Updated: 2026-05-18T06:38:19Z.

## Authority

Product and behavior authority remains:

- `docs/specs/initial_library.md`
- `docs/specs/gepa_reference_behavior.md`
- `docs/specs/gepa_aime_paper_parity.md`
- `docs/plans/2026-05-17-gepa-upstream-parity-matrix.md`
- current code/tests and emitted P8 reports

This file is a continuation ledger only. Verify every referenced artifact before
claiming completion.

## Current Goal

Make Leaven GEPA parity-or-better than pinned upstream GEPA through the public
`optimize(seed).using(Gepa...)` route, with intentional Leaven-better deltas
documented, tested, and reported.

## Current Matrix State

The parity matrix currently records the core GEPA reference-loop rows as proven
or intentional deltas. The remaining P0 row is the P8/AIME live operator proof:
a release run/report must prove profile, models, source counts/cache hash,
cache/resume behavior, search budget, baseline/optimized validation/test
numbers, and deltas versus the GEPA CAIS target.

Important currently proven rows include:

- public Layer 1 `optimize(...).using(Gepa...)` route with typed `GepaReport`;
- seed full validation before train;
- validation-Pareto parent selection and checkpointed selector state;
- epoch-shuffled minibatches and resume;
- same-case parent/child screening;
- strict improvement acceptance;
- accepted-child full validation before admission;
- per-case evaluation cache reuse and zero-cost hit accounting;
- skip-perfect/no-reflective-examples before LM work;
- upstream-style generic and AIME reflection prompt/parser snapshots;
- target-safe reflective dataset projection and hidden-target isolation;
- GEPA-specific phase events and P8 JSON event projection;
- materialized AIME cache proof: `target/leaven-aime-cache/aime.json`,
  SHA-256 `0f39c54861fd37a609d5bf397902a2086c245ebee879704dbd74b485115402c3`,
  570246 bytes, train 45, validation 45, test 30, 120/120 unique source IDs.

Do not claim DSPy-default parity. Current claims are core GEPA or
optimize-anything/AIME profile; DSPy merge and DSPy trace defaults are not
implemented as default parity.

## Live P8 Run Ledger

Current live release run directory:

```text
.leaven/release-runs/p8-aime-gepa-20260518-043717
```

Pointer file:

```text
.leaven/release-runs/latest-gepa-aime-run.txt
```

Command shape used:

```bash
set -a
source ~/plans/.env
set +a
export LEAVEN_AIME_LIVE_OPENAI=1
export LEAVEN_AIME_PROFILE=gepa-aime
export LEAVEN_AIME_CACHE=target/leaven-aime-cache/aime.json
export LEAVEN_AIME_LM_CACHE_BACKEND=eager-sqlite
export LEAVEN_AIME_RUN_DIR=.leaven/release-runs/p8-aime-gepa-20260518-043717
export LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS="${LEAVEN_OPENAI_MAX_CONCURRENT_REQUESTS:-32}"
cargo run -p p8_aime_gepa
```

Observed first attempt:

- same run id: `03ee6ad5-3234-4a24-81ac-f17a7358b275`;
- progressed to 357 search metric calls and 294 LM calls;
- admitted multiple children and ran accepted-child full validation batches;
- failed closed before final reports with OpenAI transport timeout:
  `error sending request for url (https://api.openai.com/v1/responses)`,
  `operation timed out`;
- no completed `reports/p8-aime.json` should be treated as emitted from that
  failed attempt.

Observed resume attempt with default 120s timeout:

- reused same run id;
- skipped seed validation and resumed from the run directory;
- failed again at the same search position with an OpenAI transport timeout.

Current resume attempt:

- added `LEAVEN_OPENAI_REQUEST_TIMEOUT_SECONDS=600`;
- same run id accepted resume;
- observed evaluation-cache hits with zero metric calls inside the resumed run;
- observed through resumed request count 31 / total assessment rows 150;
- still running as of this ledger update.

The 600s timeout changes the OpenAI provider fingerprint. The current run
accepted the resume, so use the emitted report to verify whether the role
fingerprints and compatibility disclosures are acceptable before claiming live
parity.

Duplicate run guard:

- a second older provider process was found using `/tmp/leaven-gepa-live-run-dir.txt`
  and run directory `.leaven/release-runs/p8-aime-gepa-20260517-213546`;
- it did not have the 600s timeout override and was appending to the legacy
  `.log` sidecar rather than the current ledger run `output.log`;
- stopped PID pairs `27627`/`27655` and respawned `56611`/`56646`;
- stopped a later bare respawned child `59403`;
- after the second stop, only the intended 600s-timeout process for
  `.leaven/release-runs/p8-aime-gepa-20260518-043717` remained.

## Existing Prior Live Artifact

Prior report worth comparing, but not sufficient as current proof:

```text
.leaven/runs/2a582001-8fb7-4cdc-9926-6054ab5a1846/reports/p8-aime.json
```

It records a real read-write live run with OpenAI roles and no provider
failures:

- profile: `gepa-aime`;
- target: `gepa_cais_aime_math_artifact`;
- baseline train: `0.6`;
- optimized train: `0.6666666666666666`;
- validation: `0.5555555555555556`;
- held-out test: `0.5666666666666667`;
- search metric calls: `472`;
- live role calls: `440`;
- live role cache policies: `read-write,read-write`;
- live role cache misses: `276`;
- provider failures: `0`.

It predates the current materialized-cache hash/report proof stack and should
not by itself close the current live release row.

## Next Actions

1. Let the current 600s-timeout resume finish or fail. Do not start a second
   competing provider run.
2. If it emits `reports/p8-aime.json`, inspect:
   - `proof_classification`;
   - `run_profile`;
   - `comparison`;
   - `dataset`/`data_source` materialized cache proof;
   - `budget`;
   - `cache`;
   - `lm_roles`;
   - `scores`;
   - `run.summary_json`;
   - provider-failure counters.
3. Update `docs/plans/2026-05-17-gepa-upstream-parity-matrix.md` only if the
   emitted report proves the live row.
4. If it fails again, record the failure here and decide whether the P8 operator
   path needs stronger retry/resume behavior, lower concurrency, or provider
   timeout defaults before spending more.
