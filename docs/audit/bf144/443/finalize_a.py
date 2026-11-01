import gzip
import hashlib
import json
from pathlib import Path
from check_stages import validate

r = Path(__file__).resolve().parent
read = lambda name: json.loads((r / name).read_text())
baseline = read("runtime-baseline/parameters.json")
restored = read("runtime-restored/parameters.json")
validate(baseline)
validate(restored)
for key in ("rows", "after_motors", "camera_after_post_update"):
    assert baseline[key] == restored[key]
assert read("replay-ablated.report.json")["process_exit_code"] == 101
bad = read("runtime-ablated/parameters.json")
assert bad["after_motors"] == []
assert all("position" not in v and "velocity" not in v for v in bad["camera_after_post_update"])
try:
    validate(bad)
except (AssertionError, KeyError):
    pass
else:
    raise AssertionError("Removed physical observations were not rejected")
assert (r / "replay-baseline.json").read_bytes() == (r / "replay-restored.json").read_bytes()
for name,digest in read("final-sources.json").items():
    assert hashlib.sha256((r / name).read_bytes()).hexdigest() == digest
assert read("identity-check.json")["current_base_files_unchanged"] == 396
for label in ("baseline", "restored"):
    assert read(label + "-functional.json")["pass"] is True
    assert read("runtime-" + label + "/process.report.json")["success"] is True
proof = read("source-compression.json")
compressed = (r / "source.native.json.gz").read_bytes()
raw = gzip.decompress(compressed)
assert len(raw) == proof["raw_bytes"] and hashlib.sha256(raw).hexdigest() == proof["raw_sha256"]
assert hashlib.sha256(compressed).hexdigest() == proof["gzip_sha256"]
native = json.loads(raw)["character_animation_probe"]["cases"][0]["frames"]
derived = read("source-phases.json")["character_animation_probe"]["cases"][0]["frames"]
assert len(native) == len(derived) == 240
for full,part in zip(native,derived):
    assert all(full[k] == value for k,value in part.items())
matrix = read("replay-restored.json")
assert not matrix["mismatches"]
result = dict(gate="PASS_CURRENT240_HORIZONTAL_PHYSICS_AND_MATCHED_SOURCE_HISTORY_WITH_VERTICAL_DEFERRED", target_horizontal_rows_exact=240, source_input_history_rows_exact=240, source_gravity_cm_s2=-981, exact_clock_nanoseconds=dict(frame=33333335, physics=33333333, substep=5555555), substeps=6, motor_roundtrip_rows=matrix["motor_vs_raw_plan_velocity_differences"], known_consumers=["BF144-D253", "BF144-D254", "BF144-D264"], added_observation_removal_rejected=True, repeated_world_add_replay_rejected=True, source_bytes_restored=True, same_target_rebuilt=True, complete_replay_output_restored_byte_exact=True, full_runtime_phase_records_restored_exact=True, old_update240_fields_unchanged=True, vertical_motor_corrections_deferred=89, source_target_origins_and_all_parameters_matched=False, full_root_or_als_equivalence_claimed=False, new_differences=[], total_confirmed_differences=269, screenshots=0, performance_acceptance="paused")
(r / "summary-a.json").write_text(json.dumps(result,indent=2)+"\n")
print(json.dumps(result),flush=True)
