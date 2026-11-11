import json
import math
from pathlib import Path


directory = Path(__file__).resolve().parent
source = json.loads((directory / "settings.native.json").read_text())
head, = [prop for prop in source["asset_properties"] if prop["name"] == "Head"]
half_lives = {prop["name"]: float(prop["exported_text"]) for prop in head["fields"]}
assert half_lives["FirstPersonPitchAngleInterpolationHalfLife"] == 0.01
assert half_lives["PitchAngleInterpolationHalfLife"] == 0.1

rows = []
for case, update_before, dilation in [("hidden_slow", False, 0.25), ("visible_slow", True, 0.25), ("hidden_normal", False, 1.0)]:
    prior = dilation if update_before else 1.0
    scaled = dilation / 60.0
    for first_person in (False, True):
        prefix = "FirstPerson" if first_person else ""
        half_life = half_lives[prefix + "PitchAngleInterpolationHalfLife"]
        angles = []
        for delta in (scaled / prior if first_person else scaled, scaled / dilation if first_person else scaled):
            x = math.log(2.0) / (half_life + 1e-8) * delta
            angles.append(45.0 * (1.0 - 1.0 / (1.0 + 1.00746054 * x + 0.45053901 * x * x + 0.25724632 * x * x * x)))
        rows.append({
            "case": case,
            "first_person": first_person,
            "previous_dilation": prior,
            "scaled_delta": scaled,
            "resumed_unscaled_delta": scaled / prior,
            "source_pitch_degrees": angles[0],
            "pitch_if_hidden_ticks_replace_history_degrees": angles[1],
        })
failures = [row for row in rows if row["source_pitch_degrees"] != row["pitch_if_hidden_ticks_replace_history_degrees"]]
assert len(failures) == 1 and failures[0]["case"] == "hidden_slow" and failures[0]["first_person"]
assert math.isclose(failures[0]["source_pitch_degrees"], 11.285648, abs_tol=1e-6)
assert math.isclose(failures[0]["pitch_if_hidden_ticks_replace_history_degrees"], 30.767437, abs_tol=1e-6)
print(json.dumps({"cases": rows, "ablation_failures": len(failures)}, indent=2))
