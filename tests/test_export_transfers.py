import json
import tempfile
import unittest
from pathlib import Path

from export_transfers import export, valid_address


class ExportTests(unittest.TestCase):
    def test_validates_exact_json_shape_and_u64_amount(self):
        address = "1" * 32
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "raw.jsonl"
            destination = root / "transfers.jsonl"
            with source.open("w", encoding="utf-8") as stream:
                for slot in range(385870151, 385870157):
                    json.dump(
                        {"@data": {"slot": slot, "transfers": [{"from": address, "to": address, "amount": str(2**64 - 1)}]}},
                        stream,
                    )
                    stream.write("\n")
            export(source, destination)
            rows = destination.read_text(encoding="utf-8").splitlines()
            self.assertEqual(len(rows), 6)
            self.assertEqual(json.loads(rows[0]), {"from": address, "to": address, "amount": 2**64 - 1})
            self.assertNotIn(" ", rows[0])

    def test_rejects_base58_strings_with_wrong_decoded_width(self):
        self.assertTrue(valid_address("1" * 32))
        self.assertFalse(valid_address("1" * 31))
        self.assertFalse(valid_address("0" * 32))


if __name__ == "__main__":
    unittest.main()
