import json
import struct
from pathlib import Path


directory = Path(__file__).resolve().parent
source = json.loads((directory / "../d149/ab-als.native.json").read_text())
timestamp, = [prop for prop in source["blueprint_cdo_properties"] if prop["name"] == "TeleportedTime"]
assert timestamp["property_class"] == "DoubleProperty" and timestamp["numeric_value"] == 0.0
window = struct.unpack("f", struct.pack("f", 0.2))[0]
cases = []
for seconds in (0.1, 0.3, 1.0):
    for explicit_event in (False, True):
        elapsed = seconds - timestamp["numeric_value"]
        source_rebase = elapsed <= window
        old_rebase = explicit_event and elapsed <= window
        cases.append({
            "game_time": seconds,
            "explicit_zero_event": explicit_event,
            "source_rebase": source_rebase,
            "rebase_with_optional_time": old_rebase,
            "source_shift_m": 0.1 if source_rebase else 0.0,
            "shift_with_optional_time_m": 0.1 if old_rebase else 0.0,
        })
failures = [case for case in cases if case["source_rebase"] != case["rebase_with_optional_time"]]
assert len(failures) == 1
assert failures[0]["game_time"] == 0.1 and not failures[0]["explicit_zero_event"]
assert 0.2 <= window < 0.200001
print(json.dumps({"default_timestamp": timestamp["numeric_value"], "inclusive_window": window,
                  "cases": cases, "optional_time_failures": len(failures)}, indent=2))
