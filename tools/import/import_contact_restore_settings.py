import argparse
import json
from pathlib import Path

from import_movement_constants import NUMBER, literal


def settings(facts):
    source = "\n".join(item["text"] for item in facts)
    names = ["ContactPositionToleranceScale", "ShapePositionToleranceScale0", "ShapePositionToleranceScaleN",
             "ShapeRotationThreshold0", "ShapeRotationThresholdN"]
    values = [literal(source, rf"\b{name}\s*=\s*FReal\({NUMBER}\)", name) for name in names]
    scale = literal(source, rf"\bToleranceScale\s*=\s*{NUMBER}f", "collision tolerance scale")
    count = int(literal(source, rf"ExpectedNumManifoldPoints\s*=\s*FMath::Min\(ManifoldPoints\.Num\(\),\s*{NUMBER}\)", "minimum points"))
    return values, scale, count


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--facts", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.facts.resolve() == args.output.resolve():
        raise ValueError("Target output must differ from source")
    values, scale, count = settings(json.loads(args.facts.read_text()))
    numbers = ", ".join(f"{value:.17g}_f64" for value in values)
    args.output.write_text(f"([{numbers}], {scale:.9g}_f32, {count}_usize)\n")


if __name__ == "__main__":
    main()
