import hashlib
import json
from pathlib import Path
from check import validate, self_test

r = Path(__file__).resolve().parent
read = lambda p: json.loads(p.read_text())
baseline = read(r / "runtime-baseline/parameters.json")
restored = read(r / "runtime-restored/parameters.json")
a = validate(baseline)
b = validate(restored)
for ordinal in (20, 21):
    for key in ("input_move", "input_blocked", "player_view_yaw", "view_yaw", "consumed_yaw", "desired_motion"):
        assert baseline["rows"][ordinal][key] == restored["rows"][ordinal][key]
for name, digest in read(r / "final-sources.json").items():
    assert hashlib.sha256((r / name).read_bytes()).hexdigest() == digest
for label in ("baseline", "restored"):
    assert read(r / (label + "-functional.json"))["pass"] is True
    assert read(r / (label + "-compile.report.json"))["success"] is True
    assert read(r / ("runtime-" + label) / "process.report.json")["success"] is True
    assert read(r / ("runtime-" + label) / "render_effects.json")["passed"] is True
assert read(r / "controls-check.json")["gate"].startswith("PASS_")
assert read(r / "identity-check.json")["gate"].startswith("PASS_")
result = dict(gate="PASS_REAL_PLAYGROUND_NATURAL_AND_CONTROLLED_VIEW_LIFECYCLE_AND_RESTORATION", baseline=a, restored=b, self_test=self_test(), source_bytes_restored=True, same_target_game_libraries=True, controlled_minimal_counterexample_restored_exactly=True, original_scene_retained=True, native_source_warm_cache_equivalence_claimed=False, natural_nonzero_cold_input_claimed=False, specific_later_eligibility_gap_cause_claimed=False, new_differences=[], screenshots=0, performance_acceptance="paused")
(r / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result), flush=True)
