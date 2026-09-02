#!/bin/sh
set -eu

usage() {
    echo "usage: $0 <release-archive.tar.gz>" >&2
    exit 2
}

[ "$#" -eq 1 ] || usage

archive=$1
checksum="$archive.sha256"
[ -f "$archive" ] || { echo "release archive is missing: $archive" >&2; exit 3; }
[ -f "$checksum" ] || { echo "release checksum is missing: $checksum" >&2; exit 3; }

for verify_tool in jq tar; do
    command -v "$verify_tool" >/dev/null 2>&1 || {
        echo "required verification tool is unavailable: $verify_tool" >&2
        exit 3
    }
done

sha256_file() {
    if command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    elif command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        echo "no SHA-256 command is available" >&2
        exit 3
    fi
}

expected_sha256=$(awk 'NR == 1 {print $1}' "$checksum")
actual_sha256=$(sha256_file "$archive")
[ "$actual_sha256" = "$expected_sha256" ] || {
    echo "archive checksum does not match $checksum" >&2
    exit 4
}

verify_root=$(mktemp -d "${TMPDIR:-/tmp}/dts-release-verify.XXXXXX")
cleanup() {
    rm -rf "$verify_root"
}
trap cleanup EXIT HUP INT TERM
tar -xzf "$archive" -C "$verify_root"

archive_roots="$verify_root/archive-roots.txt"
find "$verify_root" -mindepth 1 -maxdepth 1 -type d | LC_ALL=C sort > "$archive_roots"
[ "$(wc -l < "$archive_roots" | tr -d ' ')" = 1 ] || {
    echo "archive must contain exactly one root directory" >&2
    exit 4
}
archive_root=$(sed -n '1p' "$archive_roots")
manifest="$archive_root/release-manifest.json"
[ -f "$manifest" ] || { echo "release-manifest.json is missing" >&2; exit 4; }

inventory="$verify_root/inventory.tsv"
jq -er '.release_manifest_schema_version == "1.0.0"' "$manifest" >/dev/null
jq -r '.files[] | [.path, (.size_bytes|tostring), .sha256] | @tsv' \
    "$manifest" > "$inventory"
while IFS="$(printf '\t')" read -r relative size_bytes sha256; do
    case "$relative" in
        /*|../*|*/../*|*/..) echo "unsafe manifest path: $relative" >&2; exit 4 ;;
    esac
    payload="$archive_root/$relative"
    [ -f "$payload" ] || { echo "manifest payload is missing: $relative" >&2; exit 4; }
    actual_size=$(wc -c < "$payload" | tr -d ' ')
    [ "$actual_size" = "$size_bytes" ] || {
        echo "payload size differs: $relative" >&2
        exit 4
    }
    [ "$(sha256_file "$payload")" = "$sha256" ] || {
        echo "payload checksum differs: $relative" >&2
        exit 4
    }
done < "$inventory"

for required in \
    CHANGELOG.md LICENSE-APACHE LICENSE-MIT THIRD_PARTY_LICENSES.json \
    compatibility-owners.json \
    examples/compose-raw-grayscale.json examples/compose-raw-rgb.json \
    examples/compose-metadata-private-sequence.json \
    examples/compose-multi-instance-reference.json \
    examples/assemble-structural.json schemas/cli-success-envelope.schema.json \
    schemas/cli-error-envelope.schema.json
do
    [ -f "$archive_root/$required" ] || {
        echo "required release payload is missing: $required" >&2
        exit 4
    }
done

binary="$archive_root/bin/synth-dicom-gen"
[ -x "$binary" ] || { echo "release binary is not executable" >&2; exit 4; }
version=$($binary version --format json)
capabilities=$($binary capabilities --format json)
printf '%s' "$version" | jq -e --slurpfile manifest "$manifest" \
    '.result == $manifest[0].version_result' >/dev/null
printf '%s' "$capabilities" | jq -e --slurpfile manifest "$manifest" \
    '.result == $manifest[0].capabilities_result' >/dev/null
printf '%s' "$version" | jq -e --slurpfile manifest "$manifest" \
    '.result.target == $manifest[0].target and
     .result.product_resources.resource_set_sha256 ==
       $manifest[0].capabilities_result.product_resources.resource_set_sha256' >/dev/null

printf 'archive=%s\n' "$archive"
printf 'sha256=%s\n' "$actual_sha256"
printf 'source_revision=%s\n' "$(jq -r '.source.revision' "$manifest")"
printf 'target=%s\n' "$(jq -r '.target' "$manifest")"
printf 'verification=passed\n'
