import gzip
import json
import struct
from pathlib import Path


directory = Path(__file__).resolve().parent
root = directory.parents[3]
manifest = json.loads((root / "assets/ALS/Animations/als_notify_manifest.json").read_bytes())
defaults = {
    "LocomotionModeEquals": '(TagName="Als.LocomotionMode.InAir")',
    "RotationModeEquals": '(TagName="Als.RotationMode.Aiming")',
    "StanceEquals": '(TagName="Als.Stance.Crouching")',
}
cases = [
    ("command", 1.0, None, False, False, True),
    ("blocked_command", 1.0, None, True, False, False),
    ("override", 0.0, 1.0, False, False, True),
    ("blocked_override", 0.0, 1.0, True, False, False),
    ("neutral_override", 1.0, 0.0, False, False, False),
    ("aim", 0.0, None, True, True, True),
    ("below_normalization_threshold", 0.00005, None, False, False, False),
]
paths = [directory / f"{kind}.native.json.gz" for kind in ["low", "high"]]
paths += sorted((directory.parent / "d183").glob("mantle-low-*.native.json.gz"))
reports = {}
for path in paths:
    with gzip.open(path) as stream:
        native = json.load(stream)
    asset = native["asset_path"]
    montage = next(clip for clip in manifest["clips"] if clip["source_asset"] == asset)
    notify = next(event for event in montage["events"] if event["class"] == "AlsAnimNotifyState_EarlyBlendOut")
    instance = next(item for item in native["asset_objects"] if item["class_path"].endswith("AlsAnimNotifyState_EarlyBlendOut"))
    props = {prop["name"]: prop["exported_text"] for prop in instance["properties"]}
    for key in ["bCheckInput", "bCheckLocomotionMode", "bCheckRotationMode", "bCheckStance"]:
        assert props[key] == "True" and notify["parameters"].get(key, True)
    for key, default in defaults.items():
        assert props[key] == default
    duration = struct.unpack("<f", struct.pack("<f", float(props["BlendOutDuration"])))[0]
    assert notify["parameters"]["BlendOutDuration"] == duration
    rows = []
    for name, command, override, blocked, aiming, expected in cases:
        requested = command if override is None else override
        acceleration_ratio = 0.0 if blocked else requested
        has_input = acceleration_ratio ** 2 >= 1e-8
        actual = has_input or aiming
        legacy = requested ** 2 > 1e-8 or aiming
        without_normalization = acceleration_ratio != 0.0 or aiming
        assert actual == expected
        rows.append({"case": name, "old_stopped": legacy, "stopped": actual,
                     "stopped_without_normalization": without_normalization})
    clocks = native.get("montage_clock_probe")
    weights = None
    if clocks is not None:
        weights = {sample["name"]: sample["frames"][-1]["weight"] for sample in clocks["samples"]}
        assert weights["blocked"] == 1.0 and 0.0 < weights["held"] < 1.0
        assert all(sample["frames"][0]["weight"] == 1.0 for sample in clocks["samples"])
    reports[asset] = {"native_weights": weights, "cases": rows}
assert len(reports) == 6
old = sum(row["old_stopped"] != row["stopped"] for report in reports.values() for row in report["cases"])
no_normalization = sum(row["stopped_without_normalization"] != row["stopped"]
                       for report in reports.values() for row in report["cases"])
assert (old, no_normalization) == (12, 6)
print(json.dumps({"legacy_mismatches": old, "without_normalization_mismatches": no_normalization,
                  "montages": reports}, indent=2))
