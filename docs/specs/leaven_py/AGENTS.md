# AGENTS — docs/specs/leaven_py

## Boundary

This subtree is no longer the executable Python SDK project. The real in-repo
Python SDK project is `sdk/python`.

Remaining content here is provenance only: vendored reference repositories,
agent-context notes, and local generated caches from the former scaffold
location. Do not add package source, examples, tests, project metadata, or
runtime behavior here.

## Rules

- Use `sdk/python` for Python SDK code, examples, tests, dependency changes,
  and verification.
- Treat `repos/` and `docs/agent-context/` as read-only research input unless
  the task is explicitly to update vendored provenance.
- Do not import from this subtree in `sdk/python` runtime code or examples.
- If a current doc still points at `docs/specs/leaven_py` as the runnable
  package location, update it to `sdk/python` in the same change.
