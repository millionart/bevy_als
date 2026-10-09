import argparse
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "animgraph_authoring"))
from ue_animation_asset_resolver import object_property


def source_intensity(world):
    lights = [item for item in world["asset_objects"] if item["class_path"] == "/Script/Engine.DirectionalLightComponent"]
    if len(lights) != 1:
        raise ValueError("Playground source directional light is missing or ambiguous")
    return float(object_property(lights[0], "Intensity"))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--world", type=Path, required=True)
    parser.add_argument("--scene", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    world = json.loads(args.world.read_text(encoding="utf-8-sig"))
    intensity = source_intensity(world)
    text = args.scene.read_text(encoding="utf-8")
    updated, count = re.subn(r"sun_intensity:\s*Some\([^)]*\)", f"sun_intensity: Some({intensity})", text)
    if count != 1:
        raise ValueError("Playground target directional light is missing or ambiguous")
    if args.check:
        if text != updated:
            raise ValueError("Playground light differs from source")
    else:
        args.scene.write_text(updated, encoding="utf-8")
    print(json.dumps({"source_lux": intensity, "check": args.check}))


if __name__ == "__main__":
    main()
