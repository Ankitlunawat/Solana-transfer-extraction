# Solana Transfer Extraction with Substreams

A Rust-based [Substreams](https://substreams.streamingfast.io/) module that extracts native SOL and SPL-token transfer instructions from a defined range of Solana mainnet blocks. It produces a compiled `.spkg` package and exports **one JSON object per transfer** in the format requested by the CLR3 Ventures assignment.

**Assignment:** [CLR3 Ventures — Solana Transfer Extraction](https://github.com/CLR3-Ventures/substreams-assignment-1/blob/main/.unifiedhiring/task.md)  
**Network:** `solana-mainnet-beta`  
**Requested slots:** `385870151` through `385870156` (inclusive)  
**Substreams module:** `map_transfers`  
**Package:** `solana-transfer-extraction-v0.1.0.spkg`

## Results

The project was built and executed in **Ubuntu 24.04 on WSL2**, using Rust and the Substreams CLI. The reported streaming run completed successfully with **6 blocks processed and 6 received**, and the exporter emitted **3,974 transfer records**.

| Solana slot | Transfers extracted |
| --- | ---: |
| 385870151 | 542 |
| 385870152 | 581 |
| 385870153 | 961 |
| 385870154 | 596 |
| 385870155 | 474 |
| 385870156 | 820 |
| **Total** | **3,974** |

The three existing Rust unit tests passed. The WebAssembly release build and SPKG packaging also succeeded. These counts are from the project's execution logs; they have **not** been independently reconciled against a separate Solana RPC or block explorer.

## Required output

Each line of `transfers.jsonl` is a separate JSON object with exactly these three properties:

```json
{"from":"8ryouP8NbMv2EKL9V5xiAq5EEt5hKwnT9DDZHjqgTTWR","to":"DduRAGKHQ5AN1BR3HGUW7ymL8MrTao7sQwZyoYCeSU69","amount":2829556537}
```

- `from`: base58-encoded source address
- `to`: base58-encoded destination address
- `amount`: **integer** in the asset's smallest units (lamports for SOL; base units for tokens), not a floating-point display value

> The protobuf JSON representation of `uint64` amounts may contain strings. The Python exporter converts these to JSON integers while preserving precision.

## How it works

1. Substreams reads Solana blocks as `sf.solana.type.v1.Block`.
2. `src/lib.rs` iterates transactions and resolves account indexes, including addresses loaded via address lookup tables.
3. The module visits outer instructions and inner/CPI instructions.
4. `src/decoder.rs` recognizes System Program transfer instructions, SPL Token transfers, and Token-2022 transfers, and reads their raw `u64` amounts.
5. For token transfers, the implementation attempts to use owner addresses derived from token balances and account-initialization instructions. If an owner is unavailable, it falls back to the token account's address.
6. The map returns a protobuf `Transfers` message containing the slot and repeated `Transfer` records.
7. `export_transfers.py` converts the Substreams JSONL envelope to **one transfer per line**, validates fields and amounts, and checks that all six requested slots are represented.

The project extracts **transfer instructions**, not a net change in account balances. It does not report transaction fees, staking rewards, mints, burns, or account-creation funding as transfers.

### Important limitations

- `src/lib.rs` now skips transactions with `meta.err` set, so failed transactions are not emitted as completed transfers. The updated source still requires a fresh WASM/SPKG build and a new six-block run to regenerate the checked-in outputs.
- System Program `TransferWithSeed` decoding now verifies the bincode seed length and complete serialized owner key before extracting amounts. A new regression test covers truncated/malformed inputs.
- **Artifact freshness:** The Rust fixes in this repository are newer than the published SPKG and JSONL files. Re-run `bash run.sh` in Ubuntu and commit the regenerated artifacts before final submission.
- SPL-token ownership is not guaranteed to be resolved for every account. The fallback may be a **token-account address**, not an owner's wallet address.
- SOL and token amounts are combined in the same three-field output format because the assignment does not include an asset identifier. Their units must not be interpreted as interchangeable.
- Three unit tests and successful live streaming are evidence of a working pipeline, but **not an independent accuracy audit** of every transfer.

## Repository layout

```text
solana-transfer-extraction/
├── Cargo.toml                 # Rust package and dependencies
├── substreams.yaml            # Substreams manifest
├── proto/
│   └── transfers.proto        # Protobuf messages
├── src/
│   ├── lib.rs                 # map_transfers entry point
│   ├── decoder.rs             # Instruction decoding and tests
│   └── output.rs              # Protobuf-compatible Rust structs
├── export_transfers.py        # JSONL transformation and validation
├── run.sh                     # Test, build, stream and export workflow
├── README.md
├── solana-transfer-extraction-v0.1.0.spkg  # Generated package
├── raw.jsonl                  # Generated Substreams envelopes
└── transfers.jsonl            # Generated submission-ready transfers
```

Generated files may be absent from a source-only checkout. The repository includes the compiled SPKG and JSONL artifacts from the previous successful run. After changing the Rust source, rebuild and regenerate these artifacts so they correspond to the latest commit.

## Prerequisites

- Linux or **Windows 10/11 with WSL2 and Ubuntu 24.04**
- Rust toolchain (`rustc`, `cargo`, `rustup`)
- Rust target `wasm32-unknown-unknown`
- [Substreams CLI](https://docs.substreams.dev/how-to-guides/installing-the-cli)
- Python 3 for the JSONL exporter
- An authenticated Substreams provider API key with Solana mainnet access

On Ubuntu:

```bash
sudo apt update
sudo apt install -y curl build-essential git python3 ca-certificates

# Install Rust if necessary: https://rustup.rs/
rustup target add wasm32-unknown-unknown

rustc --version
cargo --version
substreams --version
```

If working in Windows, open the repository in VS Code through WSL (`code .`) so that Rust compiles on the Linux filesystem.

## Build

From the directory containing `Cargo.toml` and `substreams.yaml`:

```bash
# Unit tests
cargo test

# Build the WebAssembly module
cargo build --release --target wasm32-unknown-unknown

# Create the package (without unsupported --output flag)
substreams pack substreams.yaml

# Inspect the generated package
substreams info ./solana-transfer-extraction-v0.1.0.spkg
```

Alternatively, using the checked-in `run.sh`:

```bash
bash run.sh test
bash run.sh build
```

**Script compatibility:** The checked-in `run.sh` already uses the supported packaging command and the correct hyphenated SPKG filename.

## Authenticate and stream

Authenticate with the provider, keeping the API key private:

```bash
substreams auth
```

The Graph Market browser approval flow can be used to authorize a CLI key. Do **not** commit `.substreams.env` or paste access tokens in issues or the README.

Execute the package against the **six required slots**:

```bash
substreams run \
  ./solana-transfer-extraction-v0.1.0.spkg \
  map_transfers \
  -e mainnet.sol.streamingfast.io:443 \
  --start-block 385870151 \
  --stop-block 385870157 \
  -o jsonl > raw.jsonl
```

`--stop-block` is exclusive, so `385870157` includes slot `385870156`. If your provider requires a different endpoint, replace the `-e` value with the endpoint supplied by the provider.

Export one JSON record per transfer:

```bash
python3 export_transfers.py raw.jsonl transfers.jsonl
```

The exporter prints per-slot transfer counts to stderr. If one of the six slots is missing, it raises an error instead of silently producing an incomplete submission.

### Validate exported output

```bash
wc -l transfers.jsonl
head -n 5 transfers.jsonl

python3 - <<'PY'
import json

count = 0
with open('transfers.jsonl', encoding='utf-8') as f:
    for line in f:
        obj = json.loads(line)
        assert set(obj) == {'from', 'to', 'amount'}
        assert isinstance(obj['from'], str) and obj['from']
        assert isinstance(obj['to'], str) and obj['to']
        assert type(obj['amount']) is int
        assert 0 <= obj['amount'] < 2**64
        count += 1
print(f'Validated {count} records')
PY
```

For the reported successful run, `wc -l transfers.jsonl` returned **3,974**. The output shown above is a sample from the resulting file, not a fabricated fixture.

## Testing and verification

Three Rust unit tests passed:

| Test | Purpose |
| --- | --- |
| `seeded_transfer_uses_recipient_after_signing_base` | Checks the account-index position for seeded native transfers |
| `checked_token_transfer_requires_decimals_and_keeps_full_amount` | Checks checked-token transfer decoding and full `u64` amount handling |
| `resolves_temporary_token_account_owner` | Checks token owner extraction from account initialization |

Recorded execution summary:

```text
Processed Blocks: 6 blocks
Received Blocks: 6 blocks
Completed successfully

385870151: 542 transfers
385870152: 581 transfers
385870153: 961 transfers
385870154: 596 transfers
385870155: 474 transfers
385870156: 820 transfers
```

## Submission artifacts

- **Source:** `Cargo.toml`, `src/`, `proto/`, `substreams.yaml`, `export_transfers.py`, `run.sh`
- **Compiled package:** `solana-transfer-extraction-v0.1.0.spkg`
- **Transfer output:** `transfers.jsonl`
- **Optional diagnostic output:** `raw.jsonl`
- **Documentation:** this `README.md`

Do not include private credentials, `.substreams.env`, or local build caches (`target/`).

## References

- [Assignment instructions](https://github.com/CLR3-Ventures/substreams-assignment-1/blob/main/.unifiedhiring/task.md)
- [Substreams documentation](https://docs.substreams.dev/)
- [Substreams CLI releases](https://github.com/streamingfast/substreams/releases)
- [Solana documentation](https://solana.com/docs)
