import argparse
import gzip
import json
from pathlib import Path


def samples(reports):
    result = []
    for report in reports:
        for sample in report["physics_contact_probe"]["samples"]:
            value = {key: sample[key] for key in ("input", "geometry", "points", "next_points", "restore_allowed", "restored")}
            if "other_geometry" in sample:
                other = sample["other_geometry"]
                value["other_geometry"] = {key: other[key] for key in ("kind", "bounds_min_cm", "bounds_max_cm", "shape_to_body")}
            result.append(value)
    return {"samples": result}


def read(path):
    raw = path.read_bytes()
    return json.loads(gzip.decompress(raw) if path.suffix == ".gz" else raw)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, nargs="+", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in {path.resolve() for path in args.source}:
        raise ValueError("Target output must differ from source")
    args.output.write_text(json.dumps(samples([read(path) for path in args.source]), indent=2) + "\n")


if __name__ == "__main__":
    main()
