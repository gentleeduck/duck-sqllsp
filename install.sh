#!/usr/bin/env bash
# Build + install duck-sqllsp into ~/.local/bin in one shot.
# Workaround for limited workspace disk -- cargo install uses
# /tmp/sqlbin staging then copies to ~/.local/bin.
#
# Usage: ./install.sh
set -euo pipefail

# Kill any running server so the binary slot is free for write.
pkill -9 -f 'duck-sqllsp server' 2>/dev/null || true
sleep 1

STAGE=/tmp/sqlbin
DEST="$HOME/.local/bin/duck-sqllsp"

cargo install --path dsl-cli --root "$STAGE" --force
cp "$STAGE/bin/duck-sqllsp" "$DEST"
echo "Installed: $DEST"
ls -la "$DEST"
