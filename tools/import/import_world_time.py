import argparse
import gzip
import json
import math
from pathlib import Path


def generate(source_path, output_path):
    if source_path.resolve() == output_path.resolve():
        raise ValueError("Source and output must be distinct files")
    source = source_path.read_bytes()
    if source_path.suffix == ".gz":
        source = gzip.decompress(source)
    world = json.loads(source)
    settings = [value for value in world["asset_objects"] if value["class_path"] == "/Script/Engine.WorldSettings"]
    if len(settings) != 1:
        raise ValueError("The world must have exactly one WorldSettings object")
    fields = {value["name"]: value for value in settings[0]["properties"]}
    minimum = fields["MinUndilatedFrameTime"]["numeric_value"]
    maximum = fields["MaxUndilatedFrameTime"]["numeric_value"]
    if not all(math.isfinite(value) for value in (minimum, maximum)) or not 0 < minimum <= maximum:
        raise ValueError("The source world frame limits are invalid")
    output = {"min_undilated_seconds": minimum, "max_undilated_seconds": maximum}
    output_path.write_text(json.dumps(output, indent=2) + "\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    generate(args.source, args.output)


if __name__ == "__main__":
    main()
