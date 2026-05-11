# Fuckery Extermination Today

Date: 2026-05-11

Status: seed audit package. This directory starts by recording concrete failures
already found before the broader workspace audit continues.

## Purpose

This review exists because recent implementation work repeatedly satisfied a
nearby proxy instead of the real Leaven product contract. The concrete current
failure is the AIME/GEPA path: the example exercises some public builder
mechanics, but it does not yet prove a real GEPA reflector using Leaven's trace,
evidence, graph, LM, cache, and async surfaces end to end.

The audit goal is to find every place where Leaven currently:

- exposes implementation compromises through public API;
- names fixtures/stubs as if they were real behavior;
- bypasses lower-level primitives that were built for the job;
- makes examples prove a scripted proxy instead of the library;
- hides a required user-facing capability behind a private or missing seam;
- keeps placeholder crates or placeholder strategy names in a way that can
  contaminate future design.

## Organization

- `complaints/session-user-messages-for-codex.md`: the full authored user
  message stream for this session. Read it before writing findings.
- `auditing-conventions.md`: finding format, severity rubric, and audit
  ground rules.
- `surfaces/layer-1-user/`: ordinary end-user API smells.
- `surfaces/layer-2-gepa-customizer/`: GEPA strategy and customizer
  API smells.
- `internals/layer-3-engine-author/`: engine/core optimizer-author
  and graph/evidence access smells.
- `cross-cutting/lm-and-cache-surface.md`: LM/runtime/cache-specific smells.
- `inventory/known-findings.md`: concise finding ledger seeded from current
  evidence.
- `inventory/audit-plan.md`: broader audit plan to continue after this seed
  record is committed or otherwise made durable.

## Current Root Diagnosis

The main pattern is not "missing code" in isolation. The pattern is a mismatch
between the surface that users believe they are exercising and the lower-level
path the code actually takes.

The worst current example is GEPA reflection:

- The engine has a graph-aware `Proposer<P>` with `ProposalContext`.
- GEPA does not use it for reflection.
- GEPA instead uses its own `SurfaceProposer<A, S>` trait that only sees
  artifact, surface, and part.
- The current `ReflectiveMutation` is a fixed-edit fixture but is named like a
  real reflective mutation stage.

That creates a false-positive implementation path: examples can show GEPA-like
score movement while no real reflection from traces or feedback is happening.
