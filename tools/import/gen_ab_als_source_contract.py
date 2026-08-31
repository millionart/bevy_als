import argparse
import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_SOURCE = ROOT / "target" / "bf109" / "source_native" / "AB_Als.ue.json"
DEFAULT_OUTPUT = ROOT / "docs" / "bf109" / "source_contracts" / "AB_Als.source.json"


def canonical_hash(value):
    encoded = json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def build(source):
    return {
        "source_asset": source["asset_path"],
        "source_class": source["asset_class"],
        "source_sha256": canonical_hash(source),
        "source_payload": source,
    }


def self_test():
    source = {"asset_path": "/Example", "asset_class": "Example", "graphs": []}
    contract = build(source)
    if contract["source_payload"] != source:
        raise AssertionError("source payload")
    if len(contract["source_sha256"]) != 64:
        raise AssertionError("source hash")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    self_test()
    if args.self_test:
        print("AB_Als source contract generator self-test PASS")
        return
    source = json.loads(args.source.read_text(encoding="utf-8"))
    contract = build(source)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(contract, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(
        "AB_Als source contract generated "
        f"graphs={len(source['graphs'])} "
        f"nodes={sum(len(graph['nodes']) for graph in source['graphs'])} "
        f"sha256={contract['source_sha256']}"
    )


if __name__ == "__main__":
    main()
