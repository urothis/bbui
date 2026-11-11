#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! command -v wasm-bindgen >/dev/null 2>&1; then
    echo "Install wasm-bindgen-cli matching Cargo.lock before building the web example." >&2
    exit 1
fi
bindgen_version="$(awk '$0 == "name = \"wasm-bindgen\"" { getline; gsub(/"/, "", $3); print $3; exit }' Cargo.lock)"
if [[ -z "$bindgen_version" || "$(wasm-bindgen --version)" != "wasm-bindgen $bindgen_version" ]]; then
    echo "Install wasm-bindgen-cli $bindgen_version to match Cargo.lock." >&2
    exit 1
fi
if ! command -v llvm-strip >/dev/null 2>&1; then
    echo "Install llvm-strip to remove symbol names after wasm-bindgen." >&2
    exit 1
fi

CARGO_PROFILE_RELEASE_OPT_LEVEL=z \
CARGO_PROFILE_RELEASE_LTO=thin \
CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
    cargo build --locked --release --target wasm32-unknown-unknown -p bbui --example properties_editor
wasm-bindgen \
    --target web \
    --out-dir target/web/properties_editor/pkg \
    target/wasm32-unknown-unknown/release/examples/properties_editor.wasm
llvm-strip --strip-all target/web/properties_editor/pkg/properties_editor_bg.wasm
cp web/properties_editor/index.html target/web/properties_editor/index.html
mkdir -p target/web/properties_editor/assets
cp -R assets/icons target/web/properties_editor/assets/
mkdir -p target/web/properties_editor/licenses
cp assets/fonts/Inter-OFL.txt assets/fonts/NotoSansSymbols2-LICENSE.txt target/web/properties_editor/licenses/

echo "Built target/web/properties_editor; serve target/web over HTTP."
