#!/bin/sh
set -eu

usage() {
    echo "usage: $0 CLASS BUDGET_BYTES TARGET_ROOT [OUTPUT_PATH ...]" >&2
    exit 2
}

[ "$#" -ge 3 ] || usage

class=$1
budget_bytes=$2
target_root=$3
shift 3

case "$budget_bytes" in
    ''|*[!0-9]*) echo "budget must be an unsigned integer: $budget_bytes" >&2; exit 2 ;;
esac
awk -v value="$budget_bytes" 'BEGIN { exit !(value <= 9007199254740991) }' || {
    echo "budget exceeds the exact numeric reporting range: $budget_bytes" >&2
    exit 2
}
case "$target_root" in
    /*) ;;
    *) echo "target root must be absolute: $target_root" >&2; exit 2 ;;
esac
if [ "${CARGO_TARGET_DIR:-}" != "$target_root" ]; then
    echo "target root does not exactly match CARGO_TARGET_DIR" >&2
    exit 2
fi

safe_kibibytes() {
    measured_path=$1
    if [ ! -e "$measured_path" ]; then
        echo 0
        return
    fi
    kib=$(du -sk "$measured_path" 2>/dev/null | awk 'NR == 1 { print $1 }')
    case "$kib" in
        ''|*[!0-9]*) echo "cannot measure path: $measured_path" >&2; exit 2 ;;
    esac
    awk -v kib="$kib" 'BEGIN {
        if (kib > 9007199254740991 / 1024) exit 2
        printf "%.0f\n", kib * 1024
    }' || { echo "measurement overflow for path: $measured_path" >&2; exit 2; }
}

safe_add() {
    awk -v left="$1" -v right="$2" 'BEGIN {
        if (left > 9007199254740991 - right) exit 2
        printf "%.0f\n", left + right
    }' || { echo "measurement total overflow" >&2; exit 2; }
}

now=$(date +%s)
started=${CI_COST_STARTED_EPOCH:-$now}
case "$started" in
    ''|*[!0-9]*) echo "CI_COST_STARTED_EPOCH must be an unsigned integer" >&2; exit 2 ;;
esac
elapsed=$(awk -v now="$now" -v started="$started" 'BEGIN {
    if (started > now) exit 2
    printf "%.0f\n", now - started
}') || { echo "invalid CI cost start time" >&2; exit 2; }

target_bytes=$(safe_kibibytes "$target_root")
output_bytes=0
output_artifacts=0
for output_path do
    path_bytes=$(safe_kibibytes "$output_path")
    output_bytes=$(safe_add "$output_bytes" "$path_bytes")
    if [ -f "$output_path" ]; then
        path_artifacts=1
    elif [ -d "$output_path" ]; then
        path_artifacts=$(find "$output_path" -type f \
            -exec sh -c 'echo "$#"' sh {} + 2>/dev/null \
            | awk '{ total += $1 } END { print total + 0 }')
    else
        path_artifacts=0
    fi
    output_artifacts=$(safe_add "$output_artifacts" "$path_artifacts")
done

echo "ci_cost_class=$class"
echo "ci_cost_elapsed_build_seconds=$elapsed"
echo "ci_cost_target_root=$target_root"
echo "ci_cost_target_bytes=$target_bytes"
echo "ci_cost_output_bytes=$output_bytes"
echo "ci_cost_output_artifact_count=$output_artifacts"
echo "ci_cost_disk_budget_bytes=$budget_bytes"
echo "ci_cost_budget_enforced=${CI_COST_ENFORCE:-0}"

if [ "${CI_COST_ENFORCE:-0}" = 1 ] && awk -v used="$target_bytes" -v budget="$budget_bytes" 'BEGIN { exit !(used > budget) }'; then
    echo "target storage exceeds the $class budget: $target_bytes > $budget_bytes" >&2
    exit 1
fi
