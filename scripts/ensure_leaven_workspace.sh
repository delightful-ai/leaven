#!/usr/bin/env bash
set -euo pipefail

root="$(jj root 2>/dev/null || true)"
if [[ -z "$root" ]]; then
  echo "not inside a jj workspace; refusing Leaven paper-lane command" >&2
  exit 97
fi

expected="${LEAVEN_EXPECTED_WORKSPACE_ROOT:-/Users/darin/src/personal/leaven}"
if [[ "$root" != "$expected" ]]; then
  echo "refusing Leaven paper-lane command outside main Leaven workspace: jj root is $root" >&2
  echo "expected $expected" >&2
  exit 98
fi
