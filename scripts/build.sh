#!/usr/bin/env bash
# Build script for @streamlinelabs/streamline-wasm
# Produces an npm-ready package in pkg/
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$ROOT_DIR"

echo "==> Cleaning previous builds..."
rm -rf pkg pkg-bundler

echo "==> Building web target (ESM, for browsers & CDN)..."
wasm-pack build --target web --out-dir pkg --release

# Remove wasm-pack generated package.json (we use the root one)
rm -f pkg/package.json pkg/.gitignore

echo "==> Building bundler target (for webpack/rollup/vite)..."
wasm-pack build --target bundler --out-dir pkg-bundler --release

# Copy bundler .d.ts if it has richer types
if [ -f pkg-bundler/streamline_wasm_sdk.d.ts ]; then
  cp pkg-bundler/streamline_wasm_sdk.d.ts pkg/streamline_wasm_sdk.d.ts
fi

rm -f pkg-bundler/package.json pkg-bundler/.gitignore

echo ""
echo "==> Build complete!"
echo "    Web target:     pkg/"
echo "    Bundler target: pkg-bundler/"
echo ""
echo "    To publish: npm publish --access public"
