"""Convert Substreams JSONL to one exact transfer object per line."""
import json
import sys
from pathlib import Path

START_SLOT = 385870151
END_SLOT = 385870156
BASE58 = frozenset("123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz")


def valid_address(value):
    return isinstance(value, str) and 32 <= len(value) <= 44 and set(value) <= BASE58


def export(source, destination):
    by_slot = {}
    with source.open(encoding="utf-8-sig") as stream:
        for line in stream:
            if not line.strip():
                continue
            event = json.loads(line)
            data = event.get("@data", event.get("data", event))
            if not isinstance(data, dict) or "slot" not in data:
                continue
            slot = int(data["slot"])
            if slot not in range(START_SLOT, END_SLOT + 1) or slot in by_slot:
                raise ValueError(f"Unexpected or duplicate slot: {slot}")

            rows = []
            for row in data.get("transfers", []):
                sender, recipient = row["from"], row["to"]
                raw_amount = row.get("amount", 0)
                if isinstance(raw_amount, str) and raw_amount.isascii() and raw_amount.isdecimal():
                    raw_amount = int(raw_amount)
                if type(raw_amount) is not int or not 0 <= raw_amount < 2**64:
                    raise ValueError(f"Invalid amount in slot {slot}: {raw_amount!r}")
                if not valid_address(sender) or not valid_address(recipient):
                    raise ValueError(f"Invalid address in slot {slot}")
                rows.append({"from": sender, "to": recipient, "amount": raw_amount})
            by_slot[slot] = rows

    missing = sorted(set(range(START_SLOT, END_SLOT + 1)) - by_slot.keys())
    if missing:
        raise ValueError(f"No map output for slots {missing}. Check for skipped slots or a stream error.")

    with destination.open("w", encoding="utf-8", newline="\n") as output:
        for slot, rows in sorted(by_slot.items()):
            for row in rows:
                output.write(json.dumps(row) + "\n")
            print(f"{slot}: {len(rows)} transfers", file=sys.stderr)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit("Usage: python3 export_transfers.py raw.jsonl transfers.jsonl")
    export(Path(sys.argv[1]), Path(sys.argv[2]))
