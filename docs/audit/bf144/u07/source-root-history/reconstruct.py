import argparse
import gzip
import hashlib
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    audit = Path(__file__).resolve().parent
    root = audit.parents[4]
    identity = json.loads((audit / "identity.json").read_bytes())
    whole = hashlib.sha256()
    total = 0
    with args.output.open("wb") as output:
        for part, offset in (("prefix", 0), ("suffix", identity["suffix"]["offset_uncompressed"])):
            source = identity[part]
            path = (audit if part == "prefix" else root) / source["gzip"]
            digest = hashlib.sha256()
            size = 0
            with gzip.open(path, "rb") as stream:
                stream.seek(offset)
                for block in iter(lambda: stream.read(1024 * 1024), b""):
                    output.write(block)
                    digest.update(block)
                    whole.update(block)
                    size += len(block)
            assert size == source["bytes"] and digest.hexdigest() == source["sha256"], part
            total += size
    assert total == identity["whole"]["bytes"] and whole.hexdigest() == identity["whole"]["sha256"]
    print(json.dumps({"bytes": total, "sha256": whole.hexdigest()}))


if __name__ == "__main__":
    main()
