import argparse
import gzip
import json
import math
import re
from pathlib import Path

from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE
from import_mesh_pawn_response import field


def number(text, name):
    values = re.findall(rf"\b{re.escape(name)}\s*=\s*([-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?)f?\s*;", text)
    if len(values) != 1:
        raise ValueError(f"Source margin default is missing or ambiguous: {name}")
    return float(values[0])


def parameters(facts, settings, reports):
    source = "\n".join(item["text"] for item in facts)
    defaults = {
        "p.Chaos.Collision.MarginFraction": number(source, "Chaos_Collision_MarginFraction"),
        "p.Chaos.Collision.MarginMax": number(source, "Chaos_Collision_MarginMax"),
        "p.Chaos.Collision.ConvexZeroMargin": number(source, "Chaos_Collision_ConvexZeroMargin"),
    }
    options = field(settings["asset_properties"], "SolverOptions")["fields"]
    fraction = float(field(options, "CollisionMarginFraction")["numeric_value"])
    maximum = float(field(options, "CollisionMarginMax")["numeric_value"])
    samples = []
    observed = {}
    for report in reports:
        variables = report["physics_asset"]["constraint_console_variables"]
        for name in defaults:
            if name in variables:
                value = float(variables[name])
                if name in observed and observed[name] != value:
                    raise ValueError("Native margin console values differ")
                observed[name] = value
        samples.extend(report["physics_contact_probe"]["samples"])
    overrides = defaults | observed
    if overrides["p.Chaos.Collision.MarginFraction"] >= 0:
        fraction = overrides["p.Chaos.Collision.MarginFraction"]
    if overrides["p.Chaos.Collision.MarginMax"] >= 0:
        maximum = overrides["p.Chaos.Collision.MarginMax"]
    fraction = max(0.0, fraction)
    maximum = max(0.0, maximum)
    minimum = overrides["p.Chaos.Collision.ConvexZeroMargin"]
    if not all(math.isfinite(value) for value in (fraction, maximum, minimum)) or minimum < 0:
        raise ValueError("Source collision margins are invalid")
    boxes = [sample for sample in samples if sample["geometry"]["kind"] == "box"]
    if not boxes:
        raise ValueError("Source box margin evidence is missing")
    for sample in boxes:
        shape = sample["geometry"]
        size = min(shape["maximum_cm"][axis] - shape["minimum_cm"][axis] for axis in "xyz")
        expected = max(minimum, min(size * fraction, maximum))
        if abs(sample["collision_margin_0_cm"] - expected) > 1e-5 or sample["collision_margin_1_cm"] != 0:
            raise ValueError("Source box effective margin differs from world settings")
    return fraction, maximum * SOURCE_LENGTH_TO_RUNTIME_SCALE, minimum * SOURCE_LENGTH_TO_RUNTIME_SCALE


def read(path):
    raw = path.read_bytes()
    return json.loads(gzip.decompress(raw) if path.suffix == ".gz" else raw)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--facts", type=Path, required=True)
    parser.add_argument("--settings", type=Path, required=True)
    parser.add_argument("--native", type=Path, nargs="+", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in {path.resolve() for path in [args.facts, args.settings, *args.native]}:
        raise ValueError("Target output must differ from source")
    values = parameters(read(args.facts), read(args.settings), [read(path) for path in args.native])
    args.output.write_text("[" + ", ".join(f"{value:.9g}_f32" for value in values) + "]\n")


if __name__ == "__main__":
    main()
