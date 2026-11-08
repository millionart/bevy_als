import argparse
import json
import math
import re
from pathlib import Path

from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE
from import_convex_margin_settings import read
from import_movement_constants import NUMBER, f32, literal


def settings(facts, native):
    source = "\n".join(item["text"] for item in facts)
    matches = re.findall(r"const\s+T\s+Eps\s*=\s*TEPAEntry::SelectEpsilon\([^,]+,\s*([^\)]+)\)", source)
    if len(matches) != 1:
        raise ValueError("Source EPA face tolerance is missing or ambiguous")
    factors = [float(value.strip()) for value in matches[0].split("*")]
    if len(factors) != 4 or not all(math.isfinite(value) and value > 0 for value in factors):
        raise ValueError("Source EPA face tolerance is invalid")
    area = math.prod(factors) * SOURCE_LENGTH_TO_RUNTIME_SCALE ** 4
    initial_edge = literal(source, rf"Dir\.SizeSquared\(\)\s*>\s*{NUMBER}", "initial edge tolerance") * SOURCE_LENGTH_TO_RUNTIME_SCALE ** 2
    visibility = literal(source, rf"SelectEpsilon\(1\.e-4f,\s*{NUMBER}\)", "visibility tolerance") * SOURCE_LENGTH_TO_RUNTIME_SCALE
    witness = literal(source, rf"OutPenetration\s*<\s*{NUMBER}", "witness tolerance") * SOURCE_LENGTH_TO_RUNTIME_SCALE
    maximum = int(literal(source, rf"MaxIterations\s*=\s*{NUMBER}", "EPA iteration limit"))
    flood = int(literal(source, rf"MaxIteration\s*=\s*{NUMBER}", "EPA flood limit"))
    relative = f32(float(native["physics_asset"]["constraint_console_variables"]["p.Chaos.Collision.EPAEpsilon"]))
    if not math.isfinite(relative) or relative <= 0:
        raise ValueError("Source EPA relative epsilon is invalid")
    return area, initial_edge, visibility, witness, maximum, flood, relative


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--facts", type=Path, required=True)
    parser.add_argument("--native", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in {args.facts.resolve(), args.native.resolve()}:
        raise ValueError("Target output must differ from source")
    area, initial_edge, visibility, witness, maximum, flood, relative = settings(json.loads(args.facts.read_text()), read(args.native))
    args.output.write_text(f"({area:.17g}_f64, {initial_edge:.17g}_f64, {visibility:.17g}_f64, {witness:.17g}_f64, {maximum}_usize, {flood}_usize, {relative:.17g}_f64)\n")


if __name__ == "__main__":
    main()
