#!/usr/bin/env bash
# Builds the physics crate to WASM and inlines it (base64) into hero4-prototype.html, so the page opens from disk.
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
(cd sim && cargo build --release --target wasm32-unknown-unknown --quiet)
python -I - sim/target/wasm32-unknown-unknown/release/hero_sim.wasm hero4.template.html hero4-prototype.html <<'PY'
import base64, sys
wasm, tpl, out = sys.argv[1:]
b64 = base64.b64encode(open(wasm, 'rb').read()).decode()
html = open(tpl, encoding='utf-8').read()
assert html.count('%%WASM_BASE64%%') == 1
open(out, 'w', encoding='utf-8', newline='\n').write(html.replace('%%WASM_BASE64%%', b64))
print(f'{out}: wasm {len(b64) * 3 // 4 // 1024} KB inlined')
PY
