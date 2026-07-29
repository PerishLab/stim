#!/usr/bin/env sh
set -eu

case "${1:-}" in
  -h|--help|help) set -- install --help ;;
esac

COMMAND=${1:-install}
[ $# -gt 0 ] && shift || true

CHANNEL=${STIM_CHANNEL:-stable}
VERSION=${STIM_VERSION:-}
PUBLIC_URL=${STIM_RELEASES_PUBLIC_URL:-https://releases.stim.perish.uk}
INSTALL_ROOT=${STIM_INSTALL_ROOT:-"$HOME/.local/share/stim"}
LOCAL_BIN_DIR=${STIM_LOCAL_BIN_DIR:-"$HOME/.local/bin"}

while [ $# -gt 0 ]; do
  case "$1" in
    --channel)
      CHANNEL=${2:-}
      [ -n "$CHANNEL" ] || { echo "--channel requires a value" >&2; exit 1; }
      shift 2
      ;;
    --channel=*) CHANNEL=${1#--channel=}; shift ;;
    --version)
      VERSION=${2:-}
      [ -n "$VERSION" ] || { echo "--version requires a value" >&2; exit 1; }
      shift 2
      ;;
    --version=*) VERSION=${1#--version=}; shift ;;
    --public-url)
      PUBLIC_URL=${2:-}
      [ -n "$PUBLIC_URL" ] || { echo "--public-url requires a value" >&2; exit 1; }
      shift 2
      ;;
    --public-url=*) PUBLIC_URL=${1#--public-url=}; shift ;;
    --install-root)
      INSTALL_ROOT=${2:-}
      [ -n "$INSTALL_ROOT" ] || { echo "--install-root requires a value" >&2; exit 1; }
      shift 2
      ;;
    --install-root=*) INSTALL_ROOT=${1#--install-root=}; shift ;;
    --bin-dir)
      LOCAL_BIN_DIR=${2:-}
      [ -n "$LOCAL_BIN_DIR" ] || { echo "--bin-dir requires a value" >&2; exit 1; }
      shift 2
      ;;
    --bin-dir=*) LOCAL_BIN_DIR=${1#--bin-dir=}; shift ;;
    -h|--help|help)
      printf '%s\n' \
        "stim manager" \
        "" \
        "Usage:" \
        "  manage.sh install [--channel stable|beta] [--version vX.Y.Z]" \
        "  manage.sh update  [--channel stable|beta] [--version vX.Y.Z]" \
        "  manage.sh uninstall [--version vX.Y.Z]" \
        "" \
        "install and update leave exactly one version on disk. Earlier versions" \
        "are removed once the new binary is linked and answers --version, and" \
        "each removal is named. Rolling back is install --version <older>, which" \
        "fetches that version again; released artifacts are immutable and always" \
        "retrievable."
      exit 0
      ;;
    *) echo "unknown argument: $1" >&2; exit 1 ;;
  esac
done

need_public_url() {
  [ -n "$PUBLIC_URL" ] || {
    echo "STIM_RELEASES_PUBLIC_URL or --public-url is required" >&2
    exit 1
  }
  PUBLIC_URL=${PUBLIC_URL%/}
}

validate_inputs() {
  case "$CHANNEL" in
    stable|beta) ;;
    *) echo "channel must be stable or beta" >&2; exit 1 ;;
  esac
  case "$INSTALL_ROOT" in
    /|.|..|"$HOME") echo "refusing unsafe install root: $INSTALL_ROOT" >&2; exit 1 ;;
  esac
  case "$LOCAL_BIN_DIR" in
    /|.|..|"$HOME") echo "refusing unsafe binary directory: $LOCAL_BIN_DIR" >&2; exit 1 ;;
  esac
}

validate_version() {
  case "$CHANNEL" in
    stable) pattern='^v[0-9]+\.[0-9]+\.[0-9]+$' ;;
    beta) pattern='^v[0-9]+\.[0-9]+\.[0-9]+-beta\.[0-9]+$' ;;
  esac
  printf '%s\n' "$VERSION" | grep -Eq "$pattern" || {
    echo "invalid $CHANNEL version: $VERSION" >&2
    exit 1
  }
}

platform_archive() {
  os=$(uname -s)
  arch=$(uname -m)
  case "$os:$arch" in
    Linux:x86_64|Linux:amd64) echo "stim-x86_64-unknown-linux-gnu.tar.gz" ;;
    *) echo "unsupported platform: $os $arch" >&2; exit 1 ;;
  esac
}

latest_version() {
  sed -n 's/.*"releaseVersion"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$1" | head -n 1
}

install_stim() {
  validate_inputs
  need_public_url
  tmpdir=$(mktemp -d)
  trap 'rm -rf "$tmpdir"' EXIT INT TERM
  if [ -z "$VERSION" ]; then
    curl -fsSL "$PUBLIC_URL/$CHANNEL/latest/metadata.json" -o "$tmpdir/metadata.json"
    VERSION=$(latest_version "$tmpdir/metadata.json")
    [ -n "$VERSION" ] || { echo "failed to resolve latest stim version" >&2; exit 1; }
  fi
  validate_version
  archive=$(platform_archive)
  version_url="$PUBLIC_URL/$CHANNEL/versions/$VERSION"
  curl -fsSL "$version_url/checksums.txt" -o "$tmpdir/checksums.txt"
  curl -fsSL "$version_url/$archive" -o "$tmpdir/$archive"
  (cd "$tmpdir" && sha256sum -c checksums.txt)
  rm -rf "$INSTALL_ROOT/$VERSION"
  mkdir -p "$INSTALL_ROOT/$VERSION" "$LOCAL_BIN_DIR"
  tar -xzf "$tmpdir/$archive" -C "$INSTALL_ROOT/$VERSION"
  chmod +x "$INSTALL_ROOT/$VERSION/stim"
  link="$LOCAL_BIN_DIR/stim"
  rm -f "$link"
  ln -s "$INSTALL_ROOT/$VERSION/stim" "$link"
  "$link" --version
  printf 'installed stim to %s\n' "$link"
  sweep
}

sweep() {
  swept=""
  for seat in "$INSTALL_ROOT"/*; do
    [ -d "$seat" ] || continue
    held=$(basename "$seat")
    if [ "$held" != "$VERSION" ]; then
      rm -rf "$seat"
      swept="$swept $held"
    fi
  done
  if [ -n "$swept" ]; then
    printf 'swept:%s\n' "$swept"
  fi
}

uninstall_stim() {
  validate_inputs
  if [ -n "$VERSION" ]; then
    validate_version
  fi
  link="$LOCAL_BIN_DIR/stim"
  rm -f "$link"
  printf 'removed %s\n' "$link"
  if [ -n "$VERSION" ]; then
    rm -rf "$INSTALL_ROOT/$VERSION"
    printf 'removed %s\n' "$INSTALL_ROOT/$VERSION"
  else
    rm -rf "$INSTALL_ROOT"
    printf 'removed %s\n' "$INSTALL_ROOT"
  fi
}

case "$COMMAND" in
  install|update) install_stim ;;
  uninstall) uninstall_stim ;;
  *) echo "unknown command: $COMMAND" >&2; exit 1 ;;
esac
