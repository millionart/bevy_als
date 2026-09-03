import argparse
import copy
import json
import math
import tempfile
from pathlib import Path

from coordinate_basis import source_heading_yaw_degrees_to_runtime

FORMAT = "als_rotation_yaw_speed_curve_contract_v2"
SOURCE_CURVE = "RotationYawSpeed"
TARGET_CURVE = "RotationYawSpeedTarget"
SOURCE_FIELDS = (
    "times",
    "values",
    "interps",
    "tangent_modes",
    "weight_modes",
    "arrive_tangents",
    "leave_tangents",
    "arrive_weights",
    "leave_weights",
    "pre_infinity",
    "post_infinity",
)
SIGNED_FIELDS = ("values", "arrive_tangents", "leave_tangents")
DEFAULT_ASSETS = (
    "assets/animations/RotateInPlace/A_Als_Crouch_Rotate_90_Left.gltf",
    "assets/animations/RotateInPlace/A_Als_Crouch_Rotate_90_Right.gltf",
    "assets/animations/TurnInPlace/A_Als_Crouch_Turn_90_Left.gltf",
    "assets/animations/TurnInPlace/A_Als_Crouch_Turn_90_Right.gltf",
    "assets/animations/TurnInPlace/A_Als_Crouch_Turn_180_Left.gltf",
    "assets/animations/TurnInPlace/A_Als_Crouch_Turn_180_Right.gltf",
)


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent="\t") + "\n", encoding="utf-8")


def target_yaw_speed_degrees(value):
    return source_heading_yaw_degrees_to_runtime(value)


def target_yaw_speed_derivative(value):
    return source_heading_yaw_degrees_to_runtime(value)


def curve_entries(document):
    for animation_index, animation in enumerate(document.get("animations", [])):
        extension = animation.get("extensions", {}).get("ALS_float_curves", {})
        for curve in extension.get("curves", []):
            yield animation_index, animation, extension, curve


def find_curve(document, animation_index, name):
    for index, _, _, curve in curve_entries(document):
        if index == animation_index and curve.get("name") == name:
            return curve
    return None


def source_curve_fields(curve):
    fields = {}
    for field in SOURCE_FIELDS:
        if field in curve:
            fields[field] = copy.deepcopy(curve[field])
    return fields


def validate_curve_numbers(curve):
    times = curve.get("times")
    values = curve.get("values")
    if not isinstance(times, list) or not isinstance(values, list) or len(times) != len(values):
        raise ValueError("source yaw speed curve is invalid")
    for field in SIGNED_FIELDS:
        values = curve.get(field)
        if values is None:
            continue
        if not isinstance(values, list) or not all(
            isinstance(value, (int, float)) and math.isfinite(value) for value in values
        ):
            raise ValueError("source yaw speed curve contains invalid values")


def source_curve_matches_record(curve, record):
    return source_curve_fields(curve) == {
        field: record.get(field)
        for field in SOURCE_FIELDS
        if field in record
    } and set(source_curve_fields(curve)) == {
        field for field in SOURCE_FIELDS if field in record
    }


def target_curve_from_source(source):
    target = copy.deepcopy(source)
    target["name"] = TARGET_CURVE
    for field in SIGNED_FIELDS:
        if field in target:
            target[field] = [target_yaw_speed_derivative(value) for value in target[field]]
    return target


def source_record(path, document):
    animations = []
    for animation_index, animation, _, curve in curve_entries(document):
        if curve.get("name") != SOURCE_CURVE:
            continue
        validate_curve_numbers(curve)
        animations.append(
            {
                "index": animation_index,
                "name": animation.get("name", ""),
                **source_curve_fields(curve),
            }
        )
    if not animations:
        raise ValueError("source yaw speed curve is missing")
    return {"path": path, "animations": animations}


def source_contract(root, assets):
    values = []
    for asset in assets:
        path = root / asset
        values.append(source_record(asset, read_json(path)))
    return {
        "format": FORMAT,
        "source_curve": SOURCE_CURVE,
        "target_curve": TARGET_CURVE,
        "assets": values,
    }


def contract_issues(root, contract):
    issues = []
    if contract.get("format") != FORMAT:
        return ["yaw speed curve contract format is invalid"]
    if contract.get("source_curve") != SOURCE_CURVE or contract.get("target_curve") != TARGET_CURVE:
        return ["yaw speed curve contract names are invalid"]
    assets = contract.get("assets")
    if not isinstance(assets, list) or not assets:
        return ["yaw speed curve contract assets are invalid"]
    for entry in assets:
        path_value = entry.get("path") if isinstance(entry, dict) else None
        if not isinstance(path_value, str) or not path_value:
            issues.append("yaw speed curve contract asset path is invalid")
            continue
        path = root / path_value
        if not path.is_file():
            issues.append(f"yaw speed curve target asset is missing: {path_value}")
            continue
        document = read_json(path)
        for animation in entry.get("animations", []):
            if not isinstance(animation, dict):
                issues.append(f"yaw speed curve animation is invalid: {path_value}")
                continue
            index = animation.get("index")
            if not isinstance(index, int):
                issues.append(f"yaw speed curve contract entry is invalid: {path_value}")
                continue
            source = find_curve(document, index, SOURCE_CURVE)
            target = find_curve(document, index, TARGET_CURVE)
            if source is None or not source_curve_matches_record(source, animation):
                issues.append(f"yaw speed source curve differs: {path_value}:{index}")
                continue
            if not isinstance(target, dict):
                issues.append(f"yaw speed target curve is missing: {path_value}:{index}")
                continue
            if target != target_curve_from_source(source):
                issues.append(f"yaw speed target curve differs: {path_value}:{index}")
    return issues


def apply_contract(root, contract):
    if (
        contract.get("format") != FORMAT
        or contract.get("source_curve") != SOURCE_CURVE
        or contract.get("target_curve") != TARGET_CURVE
        or not isinstance(contract.get("assets"), list)
    ):
        raise ValueError("yaw speed curve contract is invalid")
    for entry in contract["assets"]:
        path = root / entry["path"]
        document = read_json(path)
        for animation in entry["animations"]:
            source = find_curve(document, animation["index"], SOURCE_CURVE)
            if source is None or not source_curve_matches_record(source, animation):
                raise ValueError(f"yaw speed source curve differs: {entry['path']}:{animation['index']}")
            target = target_curve_from_source(source)
            replaced = False
            for index, _, extension, curve in curve_entries(document):
                if index == animation["index"] and curve.get("name") == TARGET_CURVE:
                    curves = extension["curves"]
                    curves[curves.index(curve)] = target
                    replaced = True
                    break
            if not replaced:
                document["animations"][animation["index"]]["extensions"]["ALS_float_curves"]["curves"].append(target)
        write_json(path, document)


def run_self_test():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        asset = root / "assets" / "curve.gltf"
        write_json(
            asset,
            {
                "animations": [
                    {
                        "name": "rotation",
                        "extensions": {
                            "ALS_float_curves": {
                                "curves": [
                                    {
                                        "name": SOURCE_CURVE,
                                        "times": [0.0, 1.0],
                                        "values": [-90.0, 90.0],
                                        "arrive_tangents": [-7.0, 5.0],
                                        "leave_tangents": [-4.0, 11.0],
                                    }
                                ]
                            }
                        },
                    }
                ]
            },
        )
        contract = source_contract(root, ["assets/curve.gltf"])
        apply_contract(root, contract)
        assert not contract_issues(root, contract)
        document = read_json(asset)
        target = find_curve(document, 0, TARGET_CURVE)
        assert target["values"] == [90.0, -90.0]
        assert target["arrive_tangents"] == [7.0, -5.0]
        assert target["leave_tangents"] == [4.0, -11.0]
        document["animations"][0]["extensions"]["ALS_float_curves"]["curves"][0]["values"][0] = 0.0
        write_json(asset, document)
        assert contract_issues(root, contract)
    return {
        "source_to_runtime_heading_yaw_semantics": "PASS",
        "target_curve_generated": "PASS",
        "target_curve_tangent_generated": "PASS",
        "source_curve_mutation_rejected": "PASS",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--contract", type=Path)
    parser.add_argument("--asset", action="append")
    parser.add_argument("--write", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        print(json.dumps(run_self_test(), ensure_ascii=False, indent=2))
        return 0
    if args.contract is None or (args.write == args.check):
        raise ValueError("contract and exactly one of --write or --check are required")
    root = args.root.resolve()
    contract_path = args.contract if args.contract.is_absolute() else root / args.contract
    if args.write:
        assets = args.asset or list(DEFAULT_ASSETS)
        contract = (
            read_json(contract_path)
            if contract_path.is_file() and read_json(contract_path).get("format") == FORMAT
            else source_contract(root, assets)
        )
        apply_contract(root, contract)
        write_json(contract_path, contract)
        print(json.dumps({"contract": str(contract_path), "assets": len(contract["assets"])}))
        return 0
    if not contract_path.is_file():
        raise ValueError("yaw speed curve contract is missing")
    issues = contract_issues(root, read_json(contract_path))
    print(json.dumps({"issues": issues}, ensure_ascii=False, indent=2))
    return 0 if not issues else 1


if __name__ == "__main__":
    raise SystemExit(main())
