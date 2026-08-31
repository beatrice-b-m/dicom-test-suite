#!/bin/sh
set -eu

usage() {
    echo "usage: $0 <target-triple> <dist-directory>" >&2
    exit 2
}

[ "$#" -eq 2 ] || usage

release_target=$1
dist_directory=$2
release_features=${DTS_RELEASE_FEATURES:-}
release_binary_override=${DTS_RELEASE_BINARY:-}
allow_dirty=${DTS_RELEASE_ALLOW_DIRTY:-0}

case "$release_target" in
    *[!A-Za-z0-9._-]*|'') echo "invalid target triple: $release_target" >&2; exit 2 ;;
esac

for release_tool in cargo git jq tar; do
    command -v "$release_tool" >/dev/null 2>&1 || {
        echo "required release tool is unavailable: $release_tool" >&2
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

source_revision=$(git rev-parse HEAD)
if [ -n "$(git status --porcelain)" ]; then
    release_dirty=true
    if [ "$allow_dirty" != 1 ]; then
        echo "release archives require a clean worktree" >&2
        exit 4
    fi
else
    release_dirty=false
fi

if [ -n "$release_binary_override" ]; then
    release_binary=$release_binary_override
else
    if [ -n "$release_features" ]; then
        cargo build --release --locked --target "$release_target" \
            --no-default-features --features "$release_features"
    else
        cargo build --release --locked --target "$release_target" --no-default-features
    fi
    release_binary="target/$release_target/release/dicom-test-suite"
fi

[ -x "$release_binary" ] || {
    echo "release binary is not executable: $release_binary" >&2
    exit 4
}

version_document=$($release_binary version --format json)
capabilities_document=$($release_binary capabilities --format json)
product_version=$(printf '%s' "$version_document" | jq -er '.result.product.version')
binary_target=$(printf '%s' "$version_document" | jq -er '.result.target')
[ "$binary_target" = "$release_target" ] || {
    echo "binary target $binary_target does not match requested target $release_target" >&2
    exit 4
}
printf '%s' "$version_document" \
    | jq -e --arg features "$release_features" \
        '(.result.enabled_features | sort) ==
         ($features | split(",") | map(select(length > 0)) | sort)' >/dev/null || {
    echo "binary feature set does not match DTS_RELEASE_FEATURES" >&2
    exit 4
}

archive_name="dicom-test-suite-$product_version-$release_target"
mkdir -p "$dist_directory"
archive_path="$dist_directory/$archive_name.tar.gz"
checksum_path="$archive_path.sha256"
[ ! -e "$archive_path" ] && [ ! -e "$checksum_path" ] || {
    echo "release artifact already exists for $archive_name" >&2
    exit 4
}

staging_parent=$(mktemp -d "$dist_directory/.dts-release.XXXXXX")
cleanup() {
    rm -rf "$staging_parent"
}
trap cleanup EXIT HUP INT TERM
archive_root="$staging_parent/$archive_name"
mkdir -p "$archive_root/bin" "$archive_root/docs"

cp "$release_binary" "$archive_root/bin/dicom-test-suite"
cp LICENSE-APACHE LICENSE-MIT README.md Cargo.lock "$archive_root/"
for release_doc in \
    docs/generation-guide.md \
    docs/sdk-guide.md \
    docs/assembly-guide.md \
    docs/compatibility-policy.md \
    docs/standalone-product-status-2026-08-31.md
do
    cp "$release_doc" "$archive_root/docs/"
done

printf '%s\n' "$version_document" | jq -S . > "$archive_root/version.json"
printf '%s\n' "$capabilities_document" | jq -S . > "$archive_root/capabilities.json"
cargo metadata --locked --offline --format-version 1 \
    --filter-platform "$release_target" \
    | jq -S --arg target "$release_target" \
        '{notice_schema_version:"1.0.0", target:$target,
        packages:[.packages[] | select(.source != null) |
        {name,version,license,license_file,source}] | sort_by(.name,.version)}' \
    > "$archive_root/THIRD_PARTY_LICENSES.json"

file_inventory="$staging_parent/files.jsonl"
: > "$file_inventory"
find "$archive_root" -type f | LC_ALL=C sort | while IFS= read -r release_file; do
    relative_path=${release_file#"$archive_root/"}
    file_size=$(wc -c < "$release_file" | tr -d ' ')
    file_sha256=$(sha256_file "$release_file")
    jq -cn --arg path "$relative_path" --arg sha256 "$file_sha256" \
        --argjson size_bytes "$file_size" \
        '{path:$path,size_bytes:$size_bytes,sha256:$sha256}' >> "$file_inventory"
done

jq -S -n \
    --arg version "$product_version" \
    --arg revision "$source_revision" \
    --arg target "$release_target" \
    --arg features "$release_features" \
    --argjson dirty "$release_dirty" \
    --slurpfile version_document "$archive_root/version.json" \
    --slurpfile capabilities_document "$archive_root/capabilities.json" \
    --slurpfile files "$file_inventory" \
    '{release_manifest_schema_version:"1.0.0",
      product:{name:"dicom-test-suite",version:$version},
      source:{revision:$revision,dirty:$dirty},
      target:$target,
      enabled_features:($features | split(",") | map(select(length > 0))),
      version_result:$version_document[0].result,
      capabilities_result:$capabilities_document[0].result,
      files:$files}' > "$archive_root/release-manifest.json"

tar -C "$staging_parent" -czf "$archive_path" "$archive_name"
archive_sha256=$(sha256_file "$archive_path")
printf '%s  %s\n' "$archive_sha256" "$(basename "$archive_path")" > "$checksum_path"

echo "archive=$archive_path"
echo "checksum=$checksum_path"
echo "sha256=$archive_sha256"
