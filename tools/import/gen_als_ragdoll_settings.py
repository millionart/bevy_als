import argparse
import json
import re
import sys
from pathlib import Path

from gen_ragdoll import bool_after, expanded_text, number_after, props


ROOT = Path(__file__).resolve().parents[2]


def method(source, name):
    match = re.search(rf"(?ms)^void AAlsCharacter::{name}\b.*?^}}", source)
    if not match:
        raise ValueError(f"missing source method: {name}")
    return match.group()


def literal(source, name):
    matches = re.findall(rf"\b{name}\s*\{{\s*([0-9.]+)f?\s*\}}", source)
    if len(matches) != 1:
        raise ValueError(f"ambiguous source constant: {name}")
    return float(matches[0])


def source_motion_settings(source):
    start = method(source, "StartRagdollingImplementation")
    refresh = method(source, "RefreshRagdolling")
    frames = re.search(r"SpeedLimitFrameTimeRemaining\s*=\s*(\d+)", start)
    if not frames:
        raise ValueError("missing initial speed limit frame count")
    return {
        "minimum_initial_speed_limit_mps": literal(start, "MinSpeedLimit") * 0.01,
        "initial_speed_limit_refresh_frames": int(frames.group(1)),
        "reference_speed_mps": literal(refresh, "ReferenceSpeed") * 0.01,
        "angular_stiffness": literal(refresh, "Stiffness"),
        "stop_montage_blend_seconds": literal(start, "BlendOutDuration"),
    }


def object_path(text):
    matches = [value for value in re.findall(r"(/[A-Za-z0-9_/.]+\.[A-Za-z0-9_]+)", text) if not value.startswith("/Script/")]
    if len(matches) != 1:
        raise ValueError(f"missing source asset reference: {text}")
    return matches[0]


def field_text(asset, name):
    return expanded_text(next(entry for entry in asset["asset_properties"] if entry["name"] == name))


def montage_settings(asset, assets_root):
    values = props(asset["asset_properties"])
    tracks = values["SlotAnimTracks"]
    if 'SlotName="PostLocomotion"' not in tracks:
        raise ValueError("get-up montage does not use PostLocomotion")
    sequence = object_path(tracks.split("AnimReference=", 1)[1])
    clip = sequence.split(".", 1)[0].removeprefix("/ALS/") + ".anim.ron"
    if not (assets_root / clip).is_file():
        raise ValueError(f"missing runtime get-up clip: {clip}")
    blend_in = field_text(asset, "BlendIn")
    blend_out = field_text(asset, "BlendOut")
    if number_after(blend_in, "BlendTime") != 0.0 or "BlendOption=Cubic" not in blend_out:
        raise ValueError("unsupported get-up blend settings")
    if float(values["BlendOutTriggerTime"]) != -1.0 or values["bEnableAutoBlendOut"] != "True":
        raise ValueError("unsupported get-up auto blend-out")
    notifies = values["Notifies"]
    if "AlsAnimNotifyState_SetLocomotionAction" not in notifies:
        raise ValueError("missing get-up locomotion action notify")
    return {
        "source_montage": asset["asset_path"],
        "source_sequence": sequence,
        "clip": clip,
        "curves": clip.removesuffix(".anim.ron") + ".gltf#AlsCurves",
        "sequence_length": float(values["SequenceLength"]),
        "play_rate": float(values["RateScale"]),
        "blend_out_seconds": number_after(blend_out, "BlendTime"),
        "notify_end_clip_seconds": number_after(notifies, "Duration"),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--uasset", type=Path)
    parser.add_argument("--content-root", type=Path)
    parser.add_argument("--mount", default="/ALS")
    parser.add_argument("--source-cpp", type=Path)
    parser.add_argument("--output", type=Path, default=ROOT / "assets/ALS/Data/Character/als_ragdoll_settings.json")
    parser.add_argument("--report", type=Path, default=ROOT / "target/als_ragdoll_settings.report.json")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    assert object_path("/Script/Engine.AnimMontage'/ALS/GetUp.GetUp'") == "/ALS/GetUp.GetUp"
    fixture = "void AAlsCharacter::StartRagdollingImplementation()\n{\nauto MinSpeedLimit{200.0f}; auto BlendOutDuration{0.2f}; SpeedLimitFrameTimeRemaining = 8;\n}\nvoid AAlsCharacter::RefreshRagdolling(float dt)\n{\nauto ReferenceSpeed{1000.0f}; auto Stiffness{25000.0f};\n}"
    assert source_motion_settings(fixture) == {
        "minimum_initial_speed_limit_mps": 2.0,
        "initial_speed_limit_refresh_frames": 8,
        "reference_speed_mps": 10.0,
        "angular_stiffness": 25000.0,
        "stop_montage_blend_seconds": 0.2,
    }
    if args.self_test:
        landing = "(bStartRagdollingOnLand=False,RagdollingOnLandSpeedThreshold=1250.000000)"
        assert not bool_after(landing, "bStartRagdollingOnLand")
        assert number_after(landing, "RagdollingOnLandSpeedThreshold") * 0.01 == 12.5
        montage = {"asset_path": "/ALS/ALS/Animations/Actions/GetUp/AM_Als_GetUp_Front.AM_Als_GetUp_Front",
            "asset_properties": [{"name": name, "exported_text": value} for name, value in {
                "SlotAnimTracks": 'SlotName="PostLocomotion",AnimReference="/ALS/ALS/Animations/Actions/GetUp/A_Als_GetUp_Front.A_Als_GetUp_Front"',
                "BlendIn": "(BlendTime=0.000000,BlendOption=Cubic)",
                "BlendOut": "(BlendTime=0.300000,BlendOption=Cubic)",
                "BlendOutTriggerTime": "-1.000000", "bEnableAutoBlendOut": "True",
                "Notifies": "AlsAnimNotifyState_SetLocomotionAction,Duration=1.166667",
                "SequenceLength": "1.500000", "RateScale": "1.200000",
            }.items()]}
        converted = montage_settings(montage, ROOT / "assets")
        assert converted["clip"] == "ALS/Animations/Actions/GetUp/A_Als_GetUp_Front.anim.ron"
        assert converted["curves"] == "ALS/Animations/Actions/GetUp/A_Als_GetUp_Front.gltf#AlsCurves"
        print("ALS ragdoll settings self-test PASS")
        return
    report = {"success": False}
    try:
        if args.uasset is None:
            raise ValueError("settings uasset is required")
        sys.path.insert(0, str(ROOT / "tools/animgraph_authoring"))
        from ue_animgraph_converter import content_root_for_uasset, read_uasset_source
        content_root = content_root_for_uasset(args.uasset, args.content_root)
        reader = ROOT / "tools/run_ue_source_reader.ps1"
        print("read ragdoll settings", flush=True)
        settings, _ = read_uasset_source(args.uasset, content_root, args.mount, reader, 300, 150)
        fields = field_text(settings, "Ragdolling")
        cpp = args.source_cpp or content_root.parent / "Source/ALS/Private/AlsCharacter_Actions.cpp"
        output = source_motion_settings(cpp.read_text(encoding="utf-8-sig"))
        output["limit_initial_speed"] = bool_after(fields, "bLimitInitialRagdollSpeed")
        output["start_on_land"] = bool_after(fields, "bStartRagdollingOnLand")
        output["on_land_speed_threshold_mps"] = number_after(fields, "RagdollingOnLandSpeedThreshold") * 0.01
        for direction in ("front", "back"):
            key = "GetUp" + direction.title() + "Montage="
            reference = object_path(fields.split(key, 1)[1].split(",", 1)[0])
            prefix = args.mount.rstrip("/") + "/"
            if not reference.startswith(prefix):
                raise ValueError("get-up montage belongs to another mount")
            asset_path = content_root / (reference.split(".", 1)[0][len(prefix):] + ".uasset")
            print(f"read get-up {direction}", flush=True)
            asset, _ = read_uasset_source(asset_path, content_root, args.mount, reader, 300, 150)
            output[direction] = montage_settings(asset, ROOT / "assets")
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(output, indent=2) + "\n", encoding="utf-8")
        report.update(success=True, source_settings=settings["asset_path"])
    except (ValueError, OSError, KeyError, IndexError, StopIteration, RuntimeError) as error:
        report["error"] = str(error)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report))
    if not report["success"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
