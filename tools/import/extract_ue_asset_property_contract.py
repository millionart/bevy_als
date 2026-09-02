import argparse
import hashlib
import json
import tempfile
from pathlib import Path


FORMAT = "als_ue_asset_property_contract_v1"


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def canonical_hash(value):
    return hashlib.sha256(
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()


def contract(source, object_path, property_names):
    objects = [
        value
        for value in source.get("asset_objects", [])
        if value.get("object_path") == object_path
    ]
    if len(objects) != 1:
        raise ValueError("source asset object is missing or ambiguous")
    value = objects[0]
    properties = {item.get("name"): item for item in value.get("properties", [])}
    selected = []
    for name in property_names:
        property_value = properties.get(name)
        if not isinstance(property_value, dict):
            raise ValueError(f"source asset property is missing: {name}")
        exported = property_value.get("exported_text")
        if not isinstance(exported, str):
            raise ValueError(f"source asset property is invalid: {name}")
        selected.append({"name": name, "exported_text": exported})
    payload = {
        "format": FORMAT,
        "source_asset": source.get("asset_path"),
        "asset_object": object_path,
        "class_path": value.get("class_path"),
        "properties": selected,
    }
    payload["sha256"] = canonical_hash(payload)
    return payload


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def run_self_test():
    source = {
        "asset_path": "/Asset",
        "asset_objects": [
            {
                "object_path": "/Asset.Object",
                "class_path": "/Script/Engine.AnimSequence",
                "properties": [
                    {"name": "AdditiveAnimType", "exported_text": "AAT_LocalSpaceBase"},
                    {"name": "RefFrameIndex", "exported_text": "0"},
                ],
            }
        ],
    }
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory) / "contract.json"
        value = contract(source, "/Asset.Object", ["AdditiveAnimType", "RefFrameIndex"])
        write_json(output, value)
        assert read_json(output) == value
        assert value["sha256"] == canonical_hash({key: value[key] for key in value if key != "sha256"})
    return {"selected_property_contract": "PASS", "canonical_hash": "PASS"}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path)
    parser.add_argument("--asset-object")
    parser.add_argument("--property", action="append", dest="properties")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if (
        args.source is None
        or not args.asset_object
        or not args.properties
        or args.output is None
        or args.write == args.check
    ):
        raise ValueError("source, asset-object, property, output, and exactly one mode are required")
    value = contract(read_json(args.source), args.asset_object, args.properties)
    if args.write:
        write_json(args.output, value)
        print(json.dumps({"output": args.output.as_posix(), "properties": len(value["properties"])}))
        return 0
    if not args.output.is_file() or read_json(args.output) != value:
        raise ValueError("asset property contract differs from source")
    print(json.dumps({"output": args.output.as_posix(), "properties": len(value["properties"])}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
