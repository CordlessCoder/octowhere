#!/usr/bin/env bash
# Builds the simulator page into dist/, a directory of static files any web server can serve:
#
#   tools/ui-web/build.sh
#   python3 -m http.server -d tools/ui-web/dist      # to try it at http://localhost:8000
#
# dist/index.html also opens straight from disk.
#
# It needs a stable Rust with the wasm32-unknown-unknown target, which rust-toolchain.toml
# here asks rustup for. wasm-opt from binaryen shrinks the module further when it is on the path.
set -euo pipefail
cd "$(dirname "$0")"

cargo build --release --locked
rm -rf dist
mkdir dist
module=target/wasm32-unknown-unknown/release/ui_web.wasm
if command -v wasm-opt >/dev/null; then
    # The features Rust's wasm32-unknown-unknown target enables by default.
    wasm-opt -O3 --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
        --enable-mutable-globals --enable-reference-types --enable-multivalue \
        "$module" -o dist/octowhere-ui.wasm
else
    cp "$module" dist/octowhere-ui.wasm
fi
# For a page opened from disk, which may not fetch the module.
{
    printf 'window.OCTOWHERE_WASM = "'
    base64 -w0 dist/octowhere-ui.wasm
    printf '";\n'
} >dist/octowhere-ui.wasm.js
cp -r www/. dist/
revision=$(git describe --always --dirty --abbrev=7 2>/dev/null || echo unknown)
sed -i "s|__REVISION__|$revision|" dist/index.html
# The loading screen's progress counts the module's bytes as they arrive, decompressed.
sed -i "s|__MODULE_BYTES__|$(stat -c%s dist/octowhere-ui.wasm)|" dist/sim.js
echo "built dist/ at $revision"
