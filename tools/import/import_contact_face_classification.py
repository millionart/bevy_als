import argparse
import gzip
import json
import math
from pathlib import Path

from import_movement_constants import NUMBER, f32, literal


def settings(facts, native):
    source = "\n".join(item["text"] for item in facts)
    default = literal(source, rf"Chaos_Collision_Manifold_PlaneContactNormalEpsilon\s*=\s*{NUMBER}f", "plane tolerance")
    bias = literal(source, rf"SmallBiasToPreventFeatureFlipping\s*=\s*{NUMBER}f", "reference bias")
    variables = native["physics_asset"]["constraint_console_variables"]
    value = variables.get("p.Chaos.Collision.Manifold.PlaneContactNormalEpsilon", default)
    if isinstance(value, bool):
        raise ValueError("Native plane tolerance is invalid")
    value = float(value)
    if not math.isfinite(value) or value <= 0:
        raise ValueError("Native plane tolerance is invalid")
    return f32(value), f32(bias)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--facts", type=Path, required=True)
    parser.add_argument("--native", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in (args.facts.resolve(), args.native.resolve()):
        raise ValueError("Target output must differ from source")
    raw = args.native.read_bytes()
    if args.native.suffix == ".gz":
        raw = gzip.decompress(raw)
    values = settings(json.loads(args.facts.read_text()), json.loads(raw))
    args.output.write_text("[" + ", ".join(f"{value:.9g}_f32" for value in values) + "]\n")


if __name__ == "__main__":
    main()
