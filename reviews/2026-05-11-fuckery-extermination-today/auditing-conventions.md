# Auditing Conventions

This audit is about whether Leaven's implementation and public surface preserve
the product vision described in the specs and user alignment messages. It is
not a style cleanup.

## Required Finding Shape

Every finding must include:

- `id`: stable local finding id, such as `L1-001` or `X-004`.
- `severity`: `blocker`, `high`, `medium`, or `low`.
- `surface`: user-visible layer or internal support surface harmed.
- `evidence`: concrete repo path and line reference.
- `promised behavior`: what the API, spec, example, or name appears to
  promise.
- `actual behavior`: what the code actually does now.
- `why it matters`: how this blocks real users, optimizer authors, GEPA
  customizers, or future implementation.
- `correction direction`: the smallest honest design direction. Do not write a
  full implementation plan unless needed to make the finding unambiguous.

## Severity Rubric

- `blocker`: the current public path can claim success while bypassing the
  intended library capability, or an end-to-end product promise cannot be
  exercised honestly.
- `high`: a power-user or optimizer-author seam is missing, sync-only,
  under-contextualized, or forces forking/wrapping to do intended work.
- `medium`: the implementation is mostly usable but leaks naming, scaffolding,
  topology, or ergonomics that will mislead a reasonable implementor.
- `low`: documentation, naming, or organization creates confusion but does not
  currently block correct implementation.

## Public Versus Private Contract

Public contract means anything an ordinary user, GEPA customizer, optimizer
author, provider author, example reader, or spec implementor is expected to
touch. Internal contract means code that exists to support those surfaces.

Auditors should mark a problem when internal implementation detail leaks into a
public path, even if the leaked detail is technically correct. Examples:

- users must instantiate a cache wrapper instead of enabling cache policy;
- users must know graph trust/evidence mechanics to score a case;
- GEPA reflection names a fixed edit fixture as reflective mutation;
- examples bypass Leaven LM/runtime APIs but still claim optimizer proof.

## Scaffolding Rule

Scaffolding is allowed only when it is named and scoped as scaffolding. It is a
finding when scaffolding:

- appears in an ordinary public path;
- is re-exported by the umbrella crate as if complete;
- is used by an example as proof of a real capability;
- duplicates a lower-level primitive instead of exercising it;
- hides missing async, trace, evidence, cache, or provider behavior.

## Spec Alignment Rule

For each audited surface, compare three things:

1. governing specs under `docs/specs`;
2. public APIs and examples in the current code;
3. the user-message archive in `complaints/session-user-messages-for-codex.md`.

A finding is strongest when all three show the same intended contract and code
does something weaker.

## Non-Findings

If a suspicious item turns out to be acceptable, write a short non-finding in
the relevant doc only when it prevents future auditors from repeating the same
investigation. Include the path checked and why it is acceptable.

