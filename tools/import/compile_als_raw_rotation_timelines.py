import argparse
import json
import math
import re
from pathlib import Path

from audit_animation_transform_conversion import accessor_values, load_gltf
from capture_als_sequence_compression import sequence_inventory


ROOT = Path(__file__).resolve().parents[2]


def compile_timelines(assets):
    timelines = []
    for clip, _ in sequence_inventory(assets).values():
        descriptor = (assets / clip).read_text()
        path = re.search(r'\bpath:\s*"([^"]+)"', descriptor)[1]
        name = re.search(r'\banimation_name:\s*"([^"]+)"', descriptor)[1]
        document, buffer = load_gltf(assets / path)
        animation = next(item for item in document["animations"] if item["name"] == name)
        rotations = [animation["samplers"][channel["sampler"]]
                     for channel in animation["channels"] if channel["target"]["path"] == "rotation"]
        linear = [sampler for sampler in rotations if sampler.get("interpolation", "LINEAR") == "LINEAR"]
        if not linear:
            continue
        inputs = {sampler["input"] for sampler in linear}
        if len(linear) != len(rotations) or len(inputs) != 1:
            raise ValueError(f"Linear rotations do not share an installed timeline: {clip}")
        times = [time for time, in accessor_values(document, buffer, inputs.pop())]
        if not times or any(not math.isfinite(time) for time in times) or any(
            start >= end for start, end in zip(times, times[1:])
        ):
            raise ValueError(f"Invalid rotation timeline: {clip}")
        timelines.append((path, name, times))
    return sorted(timelines)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--assets", type=Path, default=ROOT / "assets")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    timelines = compile_timelines(args.assets)
    args.output.write_text(json.dumps(timelines, separators=(",", ":")) + "\n")


if __name__ == "__main__":
    main()
