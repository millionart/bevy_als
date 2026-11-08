import argparse
import gzip
import json
import math
import re
from pathlib import Path

from import_movement_constants import NUMBER, f32, literal


def settings(facts, native):
    source = "\n".join(item["text"] for item in facts)
    axis = literal(source, rf"CapsuleEndCapAxisNormalThreshold\s*=\s*FReal\({NUMBER}\)", "capsule axis tolerance")
    cylinder = literal(source, rf"CapsuleCylinderTolerance\s*=\s*FReal\({NUMBER}\)", "capsule cylinder tolerance")
    default = literal(source, rf"Chaos_Collision_Manifold_CapsuleMinContactDistanceFraction\s*=\s*{NUMBER}f", "capsule contact distance")
    values = re.findall(rf"const\s+FReal\s+PlaneTolerance\s*=\s*FReal\({NUMBER}\)", source)
    if len(values) != 2 or float(values[0]) != float(values[1]):
        raise ValueError("Source capsule plane tolerances differ or are missing")
    plane = float(values[0])
    variables = native["physics_asset"]["constraint_console_variables"]
    fraction = variables.get("p.Chaos.Collision.Manifold.CapsuleMinContactDistanceFraction", default)
    if isinstance(fraction, bool):
        raise ValueError("Native capsule contact distance is invalid")
    fraction = float(fraction)
    if not math.isfinite(fraction) or not math.isfinite(plane) or plane <= 0.0:
        raise ValueError("Native capsule contact settings are invalid")
    return axis, cylinder, plane, f32(fraction)


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
    args.output.write_text("[" + ", ".join(f"{value:.17g}_f64" for value in values) + "]\n", encoding="utf-8")


if __name__ == "__main__":
    main()
