#!/usr/bin/env bash
set -euo pipefail

for name in STIM_RELEASES_S3_AK STIM_RELEASES_S3_SK STIM_RELEASES_S3_BUCKET STIM_RELEASES_S3_URL STIM_RELEASES_PUBLIC_URL RELEASE_CHANNEL RELEASE_VERSION RELEASE_ROOT; do
  if [ -z "${!name:-}" ]; then
    echo "$name is required" >&2
    exit 1
  fi
done

case "$RELEASE_CHANNEL" in
  stable) pattern='^v[0-9]+\.[0-9]+\.[0-9]+$' ;;
  beta) pattern='^v[0-9]+\.[0-9]+\.[0-9]+-beta\.[0-9]+$' ;;
  *) echo "RELEASE_CHANNEL must be stable or beta" >&2; exit 1 ;;
esac
printf '%s\n' "$RELEASE_VERSION" | grep -Eq "$pattern" || {
  echo "invalid $RELEASE_CHANNEL version: $RELEASE_VERSION" >&2
  exit 1
}

public_url="${STIM_RELEASES_PUBLIC_URL%/}"
version_prefix="$RELEASE_CHANNEL/versions/$RELEASE_VERSION"
latest_prefix="$RELEASE_CHANNEL/latest"
metadata="$RELEASE_ROOT/metadata.json"

upload() {
  local path="$1"
  local key="$2"
  local type="$3"
  local cache="$4"
  AWS_ACCESS_KEY_ID="$STIM_RELEASES_S3_AK" \
  AWS_SECRET_ACCESS_KEY="$STIM_RELEASES_S3_SK" \
  AWS_DEFAULT_REGION=auto \
  AWS_EC2_METADATA_DISABLED=true \
  aws --endpoint-url "${STIM_RELEASES_S3_URL%/}" s3api put-object \
    --bucket "$STIM_RELEASES_S3_BUCKET" \
    --key "$key" \
    --body "$path" \
    --content-type "$type" \
    --cache-control "$cache" \
    --no-cli-pager >/dev/null
}

archive="stim-x86_64-unknown-linux-gnu.tar.gz"
upload "$RELEASE_ROOT/$archive" "$version_prefix/$archive" "application/gzip" "public, max-age=31536000, immutable"
upload "$RELEASE_ROOT/checksums.txt" "$version_prefix/checksums.txt" "text/plain; charset=utf-8" "public, max-age=31536000, immutable"
upload "$GITHUB_WORKSPACE/manage.sh" "$version_prefix/manage.sh" "text/x-shellscript; charset=utf-8" "public, max-age=31536000, immutable"
upload "$GITHUB_WORKSPACE/manage.sh" "$latest_prefix/manage.sh" "text/x-shellscript; charset=utf-8" "public, max-age=60, must-revalidate"

PUBLIC_URL="$public_url" VERSION_PREFIX="$version_prefix" LATEST_PREFIX="$latest_prefix" \
RELEASE_VERSION="$RELEASE_VERSION" RELEASE_CHANNEL="$RELEASE_CHANNEL" \
ARCHIVE="$archive" METADATA="$metadata" python3 <<'PY'
import datetime
import json
import os
from pathlib import Path

env = os.environ
root = Path(env["METADATA"]).parent
archive = env["ARCHIVE"]
data = {
    "version": 1,
    "channel": env["RELEASE_CHANNEL"],
    "releaseVersion": env["RELEASE_VERSION"],
    "generatedAt": datetime.datetime.now(datetime.timezone.utc).isoformat().replace("+00:00", "Z"),
    "artifacts": {
        "linuxX64": {
            "name": archive,
            "size": (root / archive).stat().st_size,
            "url": f'{env["PUBLIC_URL"]}/{env["VERSION_PREFIX"]}/{archive}',
        },
        "checksums": {
            "name": "checksums.txt",
            "url": f'{env["PUBLIC_URL"]}/{env["VERSION_PREFIX"]}/checksums.txt',
        },
    },
    "manager": f'{env["PUBLIC_URL"]}/{env["LATEST_PREFIX"]}/manage.sh',
}
Path(env["METADATA"]).write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
PY

upload "$metadata" "$version_prefix/metadata.json" "application/json; charset=utf-8" "public, max-age=31536000, immutable"
upload "$metadata" "$latest_prefix/metadata.json" "application/json; charset=utf-8" "public, max-age=60, must-revalidate"
