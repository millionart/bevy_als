import argparse
import copy
import gzip
import hashlib
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "animgraph_authoring"))
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target, definition_ron


def without_initial_globals(definition):
    result = copy.deepcopy(definition)
    for bone in result["bones"]:
        bone.pop("initial_global")
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--program", type=Path, required=True)
    parser.add_argument("--graph", type=Path, required=True)
    args = parser.parse_args()
    if len({args.source.resolve(), args.program.resolve(), args.graph.resolve()}) != 3:
        raise ValueError("Source, program and graph must be distinct files")
    source_bytes = args.source.read_bytes()
    if args.source.suffix == ".gz":
        source_bytes = gzip.decompress(source_bytes)
    source = json.loads(source_bytes)
    if source["rig_hierarchy"]["bones"] != source["rig_cdo_hierarchy"]["bones"]:
        raise ValueError("Blueprint and CDO initial bone hierarchies differ")
    generated = compile_target(compile_contract(source))
    previous = json.loads(args.program.read_text(encoding="utf-8"))
    if without_initial_globals(generated) != without_initial_globals(previous):
        raise ValueError("Generated rig differs beyond initial global transforms")
    old_definition = definition_ron(previous)
    graph = args.graph.read_text(encoding="utf-8")
    if graph.count(old_definition) != 1:
        raise ValueError("Graph must contain exactly one matching rig definition")
    changed = [old["name"] for old, new in zip(previous["bones"], generated["bones"])
        if old["initial_global"] != new["initial_global"]]
    args.program.write_text(json.dumps(generated, indent=2) + "\n", encoding="utf-8")
    args.graph.write_text(graph.replace(old_definition, definition_ron(generated), 1), encoding="utf-8")
    print(json.dumps({"source_sha256": hashlib.sha256(source_bytes).hexdigest(),
        "bones": len(generated["bones"]), "changed_bones": changed}))


if __name__ == "__main__":
    main()
