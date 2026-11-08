import argparse
import json
import math
from pathlib import Path

from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE
from import_convex_margin_settings import number, read
from import_movement_constants import f32


def search_distance(facts, native):
    source = "\n".join(item["text"] for item in facts)
    default = number(source, "Chaos_Collision_Manifold_MinFaceSearchDistance")
    variables = native["physics_asset"]["constraint_console_variables"]
    value = float(variables.get("p.Chaos.Collision.Manifold.MinFaceSearchDistance", default))
    if not math.isfinite(value):
        raise ValueError("Source face search distance is invalid")
    return f32(value) * SOURCE_LENGTH_TO_RUNTIME_SCALE


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--facts", type=Path, required=True)
    parser.add_argument("--native", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in {args.facts.resolve(), args.native.resolve()}:
        raise ValueError("Target output must differ from source")
    value = search_distance(json.loads(args.facts.read_text()), read(args.native))
    args.output.write_text(f"{value:.17g}_f64\n")


if __name__ == "__main__":
    main()
