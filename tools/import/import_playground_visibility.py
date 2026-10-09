import argparse
import json
from copy import deepcopy
from pathlib import Path


def compile_world(world):
    result = {}
    for item in world["asset_objects"]:
        if item["class_path"] != "/Script/Engine.StaticMeshComponent":
            continue
        path = item["object_path"]
        if ":PersistentLevel." not in path:
            continue
        name = path.split(":PersistentLevel.", 1)[1].split(".", 1)[0]
        if "blocking_query_channels" not in item:
            raise ValueError("Primitive query source is missing")
        value = "Visibility" in item["blocking_query_channels"]
        if name in result and result[name] != value:
            raise ValueError("Actor primitive query responses are ambiguous")
        result[name] = value
    if not result:
        raise ValueError("World primitive query source is empty")
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--world", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    world = json.loads(args.world.read_text(encoding="utf-8-sig"))
    result = compile_world(world)
    if args.self_test:
        changed = deepcopy(world)
        primitive = next(value for value in changed["asset_objects"] if value["class_path"] == "/Script/Engine.StaticMeshComponent")
        primitive.pop("blocking_query_channels")
        try:
            compile_world(changed)
        except ValueError:
            pass
        else:
            raise AssertionError("Missing query source was accepted")
        print(json.dumps({"self_test": "passed"}))
    if args.output:
        text = json.dumps(result, indent=2, sort_keys=True) + "\n"
        if args.check:
            if args.output.read_text(encoding="utf-8") != text:
                raise ValueError("Playground visibility target differs")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(text, encoding="utf-8")
        print(json.dumps({"actors": len(result), "nonblocking": sum(not value for value in result.values())}))


if __name__ == "__main__":
    main()
