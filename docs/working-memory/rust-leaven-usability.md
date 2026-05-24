# Rust Leaven Usability

Status: stub note, awaiting first conversation.
Created: 2026-05-24.

## Authority

This is a working-memory note. It is not product law and not implementation
proof. Future conversations should treat this file as the entry point for
capturing Rust-side Leaven usability friction.

## Origin

During the 2026-05-24 design conversation that produced
`docs/working-memory/leaven-py-and-acp-transport.md`, the user surfaced:

> "in Rust, Leaven is not usable. It's really fucking hard to set up, dude.
> There are so many issues and bugs involving environment setup that, like,
> I don't know, it's really rough."

This is the load-bearing context for why the Python SDK matters: if Rust is
hard AND Python is afterthought, the SDK doesn't solve the usability
problem. The Python SDK design phase (Phase 0 of
`docs/plans/2026-05-24-leaven-py-and-acp-transport.md`) explicitly absorbs
"don't repeat Rust setup failures" as design pressure.

## What this note is for

Capture, when the user has time and energy to walk through them:

- Specific environment setup bugs they've hit.
- Specific friction patterns: "I tried to do X and it took Y hours because Z."
- Specific places where the Rust API forces ceremony the user wishes was
  invisible.
- Specific examples of where docs are missing, lie, or drift from reality.
- Specific tooling failures (cargo, rust-toolchain, just, nextest, etc.) that
  recur.

The goal is to build up a concrete inventory of what makes Rust Leaven hard
to set up so:

1. The Python SDK Phase 0 design explicitly avoids repeating these failures.
2. A future Rust UX track has a real list to work from instead of "Rust is
   hard."
3. Future Leaven contributors hit fewer of the same walls.

## Not in scope

- Solving any of these problems in this slice. This note is capture, not fix.
- The Python SDK design itself (that's Phase 0 of the leaven-py plan).
- Generic "Rust is hard" complaints. Be specific.

## Next action

Wait for the user to walk through specific examples when they have the energy
and context-switch budget. Until then, this note is a placeholder marking
that the friction exists and is load-bearing for the SDK design pressure.
