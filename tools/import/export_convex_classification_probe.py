import argparse
import gzip
import json
from pathlib import Path


def convert(report):
    keys = ("input", "geometry", "other_geometry", "points", "gjk_simplex", "collision_margin_0_cm", "collision_margin_1_cm")
    return {"samples": [{key: sample[key] for key in keys} for sample in report["physics_contact_probe"]["samples"]]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.source.resolve() == args.output.resolve():
        raise ValueError("Target output must differ from source")
    raw = args.source.read_bytes()
    if args.source.suffix == ".gz":
        raw = gzip.decompress(raw)
    args.output.write_text(json.dumps(convert(json.loads(raw)), indent=2) + "\n")


if __name__ == "__main__":
    main()
