import argparse
import json
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "animgraph_authoring"))
from coordinate_basis import source_point_to_runtime
from ue_animation_asset_resolver import asset_property, object_property, source_object_path
from ue_exported_text import parse_struct, parse_list, parse_number


def compile_mesh(source):
    objects = {value["object_path"]: value for value in source["asset_objects"]}
    setup = objects[source_object_path(asset_property(source, "BodySetup"))]
    if object_property(setup, "CollisionTraceFlag") not in ("CTF_UseDefault", "CTF_UseSimpleAndComplex"):
        raise ValueError("Static convex collision requires source simple shapes")
    geometry = parse_struct(object_property(setup, "AggGeom"))
    if set(geometry) != {"ConvexElems"}:
        raise ValueError("Static convex collision requires authored convex elements")
    result = []
    for text in parse_list(geometry["ConvexElems"]):
        element = parse_struct(text)
        if element.get("Transform", "()") != "()":
            raise ValueError("Nonidentity convex element transform is unsupported")
        points = [[parse_number(parse_struct(point)[axis]) for axis in "XYZ"] for point in parse_list(element["VertexData"])]
        if len(points) < 4 or not all(math.isfinite(value) for point in points for value in point):
            raise ValueError("Authored convex vertices are invalid")
        result.append([source_point_to_runtime(point) for point in points])
    if not result:
        raise ValueError("Authored convex elements are empty")
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, nargs="+", required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    sources = [json.loads(path.read_text(encoding="utf-8-sig")) for path in args.source]
    result = {source["asset_path"].rsplit(".", 1)[1]: compile_mesh(source) for source in sources}
    if len(result) != len(sources):
        raise ValueError("Static convex source identities are ambiguous")
    if args.self_test:
        point = [100.0, 200.0, 300.0]
        assert source_point_to_runtime(point) == [1.0, 3.0, 2.0]
        changed = json.loads(json.dumps(sources[0]))
        setup = next(value for value in changed["asset_objects"] if value["class_path"] == "/Script/Engine.BodySetup")
        next(value for value in setup["properties"] if value["name"] == "CollisionTraceFlag")["exported_text"] = "CTF_UseComplexAsSimple"
        try:
            compile_mesh(changed)
        except ValueError:
            pass
        else:
            raise AssertionError("Complex-only source was accepted")
        print(json.dumps({"self_test": "passed", "meshes": len(result)}))
    if args.output:
        text = json.dumps(result, indent=2) + "\n"
        if args.check:
            if args.output.read_text(encoding="utf-8") != text:
                raise ValueError("Static convex target differs from source")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(text, encoding="utf-8")
        print(json.dumps({"meshes": len(result), "hulls": sum(len(value) for value in result.values()), "check": args.check}))


if __name__ == "__main__":
    main()
