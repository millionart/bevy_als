import argparse
import gzip
import json
import math
from pathlib import Path
import re

from import_convex_margin_settings import number
from import_mesh_pawn_response import field
from import_movement_constants import f32


def parameters(facts, settings, native):
    source = "\n".join(item["text"] for item in facts)
    options = field(settings["asset_properties"], "SolverOptions")["fields"]
    base = float(field(options, "CollisionCullDistance")["numeric_value"])
    variables = native["physics_asset"]["constraint_console_variables"]
    override = float(variables["p.Chaos.Solver.Collision.CullDistance"])
    if override >= 0:
        base = override
    inverse = float(variables.get("p.Chaos.Collision.CullDistanceReferenceSize",
                                  number(source, "Chaos_Collision_CullDistanceScaleInverseSize")))
    minimum = float(variables.get("p.Chaos.Collision.MinCullDistanceScale",
                                  number(source, "Chaos_Collision_MinCullDistanceScale")))
    velocity_multiplier = float(variables["p.Chaos.Solver.Collision.VelocityBoundsMultiplier"])
    max_velocity_expansion = float(variables["p.Chaos.Solver.Collision.MaxVelocityBoundsExpansion"])
    defaults = re.findall(r"bool\s+bChaos_Manifold_EnableGjkWarmStart\s*=\s*(true|false)\s*;", source)
    if len(defaults) != 1:
        raise ValueError("Source GJK warm start default is missing or ambiguous")
    warm = variables.get("p.Chaos.Collision.Manifold.EnableGjkWarmStart", defaults[0])
    if (not all(math.isfinite(value) for value in (base, override, inverse, minimum, velocity_multiplier, max_velocity_expansion))
            or base < 0 or velocity_multiplier < 0 or max_velocity_expansion < 0 or warm not in ("true", "false")):
        raise ValueError("Source contact lifecycle settings are invalid")
    return [f32(base), f32(inverse), f32(minimum), f32(velocity_multiplier), f32(max_velocity_expansion)], warm == "true"


def read(path):
    raw = path.read_bytes()
    return json.loads(gzip.decompress(raw) if path.suffix == ".gz" else raw)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--facts", type=Path, required=True)
    parser.add_argument("--settings", type=Path, required=True)
    parser.add_argument("--native", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in {path.resolve() for path in (args.facts, args.settings, args.native)}:
        raise ValueError("Target output must differ from source")
    values, warm = parameters(read(args.facts), read(args.settings), read(args.native))
    args.output.write_text("([" + ", ".join(f"{value:.9g}_f32" for value in values) + "], " + ("true" if warm else "false") + ")\n")


if __name__ == "__main__":
    main()
