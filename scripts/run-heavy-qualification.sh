#!/bin/sh
set -eu

usage() {
  cat <<'EOF'
Usage: scripts/run-heavy-qualification.sh [--dry-run] CLASS

CLASS is one of:
  byte-parity  Byte parity across the ordinary, stress, and legacy scope.
  all-profile  Full `all` profile union with explicit opt-in stress coverage.
  wsi          Ordinary WSI byte parity plus the reduced stress pyramid.
  stress       Stress manifest projection and streaming execution.
  all          Every R2.3 heavy entry exactly once.

--dry-run prints the exact Cargo commands without executing test bodies.
EOF
}

dry_run=0
if [ "${1:-}" = "--dry-run" ]; then
  dry_run=1
  shift
fi

if [ "$#" -ne 1 ]; then
  usage >&2
  exit 2
fi

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

run_exact() {
  harness=$1
  entry=$2
  if [ "$dry_run" -eq 1 ]; then
    printf '%s\n' "cargo test --locked --no-default-features --test $harness $entry -- --ignored --exact"
  else
    listing=$(cargo test --locked --no-default-features --test "$harness" "$entry" -- \
      --ignored --exact --list)
    matches=$(printf '%s\n' "$listing" | grep -Fxc -- "$entry: test") || matches=0
    if [ "$matches" -ne 1 ]; then
      printf 'heavy entry selection must resolve exactly once: %s (%s matches)\n' \
        "$entry" "$matches" >&2
      exit 3
    fi
    cargo test --locked --no-default-features --test "$harness" "$entry" -- --ignored --exact
  fi
}

byte_parity() {
  run_exact corpus_generation__nightly \
    case_recipe_catalog::data_first_sc_and_metadata_values_and_hashes_match_current_generator_bytes
}

all_profile() {
  run_exact cli_sdk__nonfast \
    generate_cli::generate_command_writes_all_profile_union_and_skips_planned_cases
}

wsi() {
  run_exact engine__nightly \
    wsi_direct_plan::ordinary_wsi_direct_plans_match_fresh_seed_one_bytes_and_manifest_facts
  run_exact corpus_generation__nightly \
    wsi_pyramid::stress_profile_emits_complete_three_instance_wsi_pyramid
}

stress() {
  run_exact corpus_generation__nightly \
    curated_stress_manifest::typed_stress_projection_matches_frozen_file_values_and_resources
  run_exact corpus_generation__nightly \
    curated_stress_sc_integration::all_stress_sc_cases_execute_through_private_streaming_services
}

case "$1" in
  byte-parity) byte_parity ;;
  all-profile) all_profile ;;
  wsi) wsi ;;
  stress) stress ;;
  all)
    byte_parity
    all_profile
    wsi
    stress
    ;;
  -h|--help) usage ;;
  *)
    printf 'unknown heavy qualification class: %s\n' "$1" >&2
    usage >&2
    exit 2
    ;;
esac
