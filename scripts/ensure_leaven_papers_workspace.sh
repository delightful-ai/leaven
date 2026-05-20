#!/usr/bin/env bash
set -euo pipefail

root="$(jj root 2>/dev/null || true)"
if [[ -z "$root" ]]; then
  echo "not inside a jj workspace; refusing paper-lane command" >&2
  exit 97
fi

case "$root" in
  */leaven-papers) ;;
  *)
    echo "refusing paper-lane command outside leaven-papers: jj root is $root" >&2
    echo "expected the isolated papers workspace, not the default leaven workspace" >&2
    exit 98
    ;;
esac
