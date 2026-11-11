import gzip
import itertools
import json
from pathlib import Path


directory = Path(__file__).resolve().parent
assets = {}
for facing in ["front", "back"]:
    with gzip.open(directory / f"{facing}.native.json.gz") as stream:
        source = json.load(stream)
    root_motion = next(item["exported_text"] for item in source["asset_properties"] if item["name"] == "bEnableRootMotion")
    assert root_motion == "False"
    assets[facing] = source["asset_path"]

rows = []
for facing, rate, input_held, aim in itertools.product(assets, [30, 60], [False, True], [False, True]):
    for before, after in [("getting_up", "getting_up"), ("getting_up", ""), ("", "")]:
        source_aim = aim and not before
        old_aim = aim and not after
        actual_aim = aim and not (after or before == "getting_up")
        assert source_aim == actual_aim
        rows.append({"facing": facing, "fps": rate, "input_held": input_held, "aim": aim,
                     "before_notify": before, "graph_action": after, "input_consumed": input_held and not before,
                     "aim_response": actual_aim, "legacy_aim_response": old_aim})
mismatches = [row for row in rows if row["aim_response"] != row["legacy_aim_response"]]
assert len(mismatches) == 8
assert all(row["before_notify"] == "getting_up" and row["graph_action"] == "" and not row["input_consumed"]
           for row in mismatches)
print(json.dumps({"assets": assets, "legacy_end_frame_mismatches": len(mismatches), "frames": rows}, indent=2))
