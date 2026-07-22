#!/usr/bin/env sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname "$0")/../../.." && pwd)
VERSION=${1:-${RELEASE_VERSION:-}}
TARGET=${TARGET:-x86_64-unknown-linux-gnu}
[ -n "$VERSION" ] || { echo "release version is required" >&2; exit 1; }
[ "$TARGET" = x86_64-unknown-linux-gnu ] || {
  echo "unsupported release target: $TARGET" >&2
  exit 1
}
printf '%s\n' "$VERSION" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+(-beta\.[0-9]+)?$' || {
  echo "invalid release version: $VERSION" >&2
  exit 1
}

release_root="$ROOT/dist/$VERSION"
mkdir -p "$release_root"
cargo build --release --locked -p stim --target "$TARGET"

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT INT TERM
cp "$ROOT/target/$TARGET/release/stim" "$tmpdir/stim"
chmod +x "$tmpdir/stim"
archive="stim-$TARGET.tar.gz"
tar -C "$tmpdir" -czf "$release_root/$archive" stim
(
  cd "$release_root"
  sha256sum "$archive" > checksums.txt
  sha256sum -c checksums.txt
  tar -tzf "$archive" | grep -qx stim
)
printf '%s\n' "$release_root"
