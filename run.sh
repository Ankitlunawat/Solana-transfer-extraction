#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"

mode=${1:-all}
case "$mode" in
  test|build|all) ;;
  *) echo "Usage: bash run.sh [test|build|all]" >&2; exit 2 ;;
esac

tools=(cargo)
[[ "$mode" == test ]] || tools+=(rustup substreams)
[[ "$mode" != all ]] || tools+=(python3)
for tool in "${tools[@]}"; do
  if ! command -v "$tool" >/dev/null; then
    echo "Missing $tool. Follow the Ubuntu setup in README.md." >&2
    exit 1
  fi
done

cargo test
[[ "$mode" != test ]] || exit 0
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
package=solana-transfer-extraction-v0.1.0.spkg
substreams pack substreams.yaml
substreams info "$package"
[[ "$mode" != build ]] || exit 0

if [[ -f .substreams.env ]]; then
  set -a
  source ./.substreams.env
  set +a
fi

output=$(mktemp -d ./.stream.XXXXXXXX)
trap 'rm -rf -- "$output"' EXIT
substreams run "$package" map_transfers \
  -e "${SOLANA_SUBSTREAMS_ENDPOINT:-mainnet.sol.streamingfast.io:443}" \
  --start-block 385870151 --stop-block 385870157 \
  -o jsonl > "$output/raw.jsonl"
python3 export_transfers.py "$output/raw.jsonl" "$output/transfers.jsonl"
mv -- "$output/raw.jsonl" "$output/transfers.jsonl" .
