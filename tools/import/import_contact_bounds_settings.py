import argparse
import gzip
import json
from pathlib import Path
import re


def settings(facts, native):
    source = "\n".join(item["text"] for item in facts)
    defaults = re.findall(r"bool\s+bChaos_Collision_EnableBoundsChecks\s*=\s*(true|false)\s*;", source)
    if len(defaults) != 1:
        raise ValueError("Source bounds check default is missing or ambiguous")
    variables = native["physics_asset"]["constraint_console_variables"]
    value = variables.get("p.Chaos.Collision.EnableBoundsChecks", defaults[0])
    if value not in ("true", "false"):
        raise ValueError("Native bounds check setting is invalid")
    return value == "true"


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
    value = settings(json.loads(args.facts.read_text()), json.loads(raw))
    args.output.write_text(("true" if value else "false") + "\n")


if __name__ == "__main__":
    main()
