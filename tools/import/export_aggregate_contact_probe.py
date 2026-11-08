import argparse
import gzip
import json
from pathlib import Path


def convert(report, label):
    samples = [sample for sample in report["physics_contact_probe"]["samples"] if sample["input"]["label"] == label]
    if len(samples) != 1:
        raise ValueError("Source aggregate sample is missing or ambiguous")
    sample = samples[0]
    return {key: sample[key] for key in ("input", "geometry", "other_geometry", "aggregate_contacts")}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--label", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.source.resolve() == args.output.resolve():
        raise ValueError("Target output must differ from source")
    raw = args.source.read_bytes()
    if args.source.suffix == ".gz":
        raw = gzip.decompress(raw)
    args.output.write_text(json.dumps(convert(json.loads(raw), args.label), indent=2) + "\n")


if __name__ == "__main__":
    main()
