#!/usr/bin/env bash
set -euo pipefail

package="leaven-workspace-firkin"
features="firkin-apple-vz-live"
test_binary="firkin_live_git_e2e"
test_name="live_apple_vz_product_pod_materializes_and_reads_back_git_workspaces"
profile="debug"
build=true

usage() {
    echo "usage: $0 [--profile debug|release] [--no-build] [test-args...]" >&2
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --profile)
            if [[ $# -lt 2 ]]; then
                usage
                exit 64
            fi
            case "$2" in
                debug|release)
                    profile="$2"
                    ;;
                *)
                    usage
                    exit 64
                    ;;
            esac
            shift 2
            ;;
        --release)
            profile="release"
            shift
            ;;
        --no-build)
            build=false
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        --)
            shift
            break
            ;;
        -*)
            usage
            exit 64
            ;;
        *)
            break
            ;;
    esac
done

if [[ -z "${LEAVEN_FIRKIN_LIVE_TEMPLATE_IMAGE:-}" ]]; then
    echo "LEAVEN_FIRKIN_LIVE_TEMPLATE_IMAGE must name an OCI image with git, sh, cat, find, mkdir, rm, test, and sleep" >&2
    exit 64
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
firkin_root="${FIRKIN_REPO:-/Users/darin/vendor/github.com/apple/containerization}"
entitlements="${firkin_root}/signing/vz.entitlements"

cd "${repo_root}"

if [[ ! -f "${entitlements}" ]]; then
    echo "missing Firkin VZ entitlements at ${entitlements}" >&2
    exit 66
fi

if [[ "${build}" == true ]]; then
    cargo_args=(test -q -p "${package}" --features "${features}" --test "${test_binary}")
    if [[ "${profile}" == "release" ]]; then
        cargo_args+=(--release)
    fi
    cargo_args+=(--no-run --message-format=json)
    cargo_json="$(mktemp)"
    cargo "${cargo_args[@]}" | tee "${cargo_json}" >/dev/null
    test_bin="$(
        python3 - "${test_binary}" "${cargo_json}" <<'PY'
import json
import sys

test_binary = sys.argv[1]
path = sys.argv[2]
selected = None
with open(path, "r", encoding="utf-8") as handle:
    for line in handle:
        line = line.strip()
        if not line:
            continue
        message = json.loads(line)
        target = message.get("target") or {}
        if (
            message.get("reason") == "compiler-artifact"
            and target.get("name") == test_binary
            and "test" in target.get("kind", [])
            and message.get("executable")
        ):
            selected = message["executable"]
if selected:
    print(selected)
PY
    )"
    rm -f "${cargo_json}"
else
    target_dir="${CARGO_TARGET_DIR:-target}"
    profile_dir="${target_dir}/${profile}"
    test_bin=""
    while IFS= read -r candidate; do
        test_list="$("${candidate}" --list --include-ignored 2>/dev/null || true)"
        if grep -Fq "${test_name}: " <<<"${test_list}"; then
            test_bin="${candidate}"
            break
        fi
    done < <(
        find "${profile_dir}/deps" -maxdepth 1 -type f -perm -111 -name "${test_binary}-*" \
            -exec stat -f '%m %N' {} \; \
            | sort -nr \
            | awk '{print $2}'
    )
fi

if [[ -z "${test_bin}" ]]; then
    echo "failed to locate ${profile} ${test_binary} test binary" >&2
    exit 1
fi

/usr/bin/codesign --force --sign - --timestamp=none \
    --entitlements "${entitlements}" \
    "${test_bin}"

/usr/bin/codesign -d --entitlements :- "${test_bin}" 2>&1 \
    | grep -E 'Executable=|com.apple.security.virtualization'

"${test_bin}" "${test_name}" --include-ignored --exact --nocapture "$@"
