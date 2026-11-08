import argparse
import json
import re
from pathlib import Path

from import_movement_constants import NUMBER, literal


def extract(facts):
    text = "\n".join(item["text"] for item in facts)
    small = literal(text, rf"#define\s+UE_SMALL_NUMBER\s+\({NUMBER}f?\)", "clip epsilon")
    normal = literal(text, rf"SafeNormalize\(FRealDouble Epsilon\s*=\s*{NUMBER}f?\)", "normal epsilon")
    maximum = literal(text, rf"MaxContactPointCount\s*=\s*{NUMBER}\s*;", "clip capacity")
    match = re.search(r"TArray<TArray<int32>>\s+PlaneVertices\s*\{(.*?)\};", text, re.DOTALL)
    if match is None:
        raise ValueError("Source box faces are missing")
    faces = [[int(value) for value in row.split(",")] for row in re.findall(r"\{\s*([0-9,\s]+)\}", match.group(1))]
    if len(faces) != 6 or any(len(face) != 4 or len(set(face)) != 4 or any(value < 0 or value >= 8 for value in face) for face in faces):
        raise ValueError("Source box faces are invalid")
    if maximum != int(maximum):
        raise ValueError("Source clip capacity is invalid")
    return faces, small, normal, int(maximum)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--facts", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.facts.resolve() == args.output.resolve():
        raise ValueError("Target output must differ from source")
    faces, small, normal, maximum = extract(json.loads(args.facts.read_text()))
    args.output.write_text(f"({faces!r}, {small:.9g}_f32, {normal:.9g}_f32, {maximum}_usize)\n")


if __name__ == "__main__":
    main()
