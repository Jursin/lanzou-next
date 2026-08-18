#!/usr/bin/env bash
set -euo pipefail

CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/tauri"
LINUXDEPLOY="$CACHE_DIR/linuxdeploy-x86_64.AppImage"

mkdir -p "$CACHE_DIR"

echo "==> 下载新版 linuxdeploy..."
curl -sL "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage" \
  -o "$LINUXDEPLOY.new"
chmod +x "$LINUXDEPLOY.new"

echo "==> 验证 strip 版本..."
tmpdir=$(mktemp -d)
"$LINUXDEPLOY.new" --appimage-extract --appdir "$tmpdir" >/dev/null 2>&1
"$tmpdir/squashfs-root/usr/bin/strip" --version | head -1
rm -rf "$tmpdir"

mv -f "$LINUXDEPLOY.new" "$LINUXDEPLOY"
echo "==> 完成: $LINUXDEPLOY"
