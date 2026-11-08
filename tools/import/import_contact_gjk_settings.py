import argparse
import json
import math
import re
from pathlib import Path

from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE
from import_convex_margin_settings import read
from import_movement_constants import f32


def settings(facts, native):
    source = "\n".join(item["text"] for item in facts)
    matches = re.findall(r"const\s+int32\s+MaxIterations\s*=\s*(\d+)\s*;", source)
    if len(matches) != 1 or int(matches[0]) < 2:
        raise ValueError("Source GJK iteration limit is invalid")
    values = native["physics_asset"]["constraint_console_variables"]
    if values["p.Chaos.Collision.UseGJK2"].lower() not in ("false", "0"):
        raise ValueError("Source GJK variant is unsupported")
    epsilon = f32(float(values["p.Chaos.Collision.GJKEpsilon"]))
    if not math.isfinite(epsilon) or epsilon <= 0:
        raise ValueError("Source GJK epsilon is invalid")
    return epsilon * SOURCE_LENGTH_TO_RUNTIME_SCALE, int(matches[0])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--facts", type=Path, required=True)
    parser.add_argument("--native", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in {args.facts.resolve(), args.native.resolve()}:
        raise ValueError("Target output must differ from source")
    epsilon, iterations = settings(json.loads(args.facts.read_text()), read(args.native))
    args.output.write_text(f"({epsilon:.17g}_f64, {iterations}_usize)\n")


if __name__ == "__main__":
    main()
