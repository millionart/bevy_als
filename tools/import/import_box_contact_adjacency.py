import argparse
import json
from pathlib import Path

from import_compound_contact_topology import vertex_planes
from import_contact_polygon_data import extract


def adjacency(facts):
    faces, _, _, _ = extract(facts)
    result = vertex_planes(faces, 8)
    if any(value["count"] != 3 for value in result):
        raise ValueError("Source box must have three planes at every vertex")
    return [value["planes"] for value in result]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--facts", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.facts.resolve() == args.output.resolve():
        raise ValueError("Target output must differ from source")
    args.output.write_text(repr(adjacency(json.loads(args.facts.read_text()))) + "\n")


if __name__ == "__main__":
    main()
