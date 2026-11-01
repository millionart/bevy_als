import copy
import hashlib
import json
from pathlib import Path

r = Path(__file__).resolve().parent
read = lambda name: json.loads((r / name).read_text())


def cold_contract(data):
    row = data["controller_snapshot"]
    view = row["player_view"]
    assert row["is_local_controller"] is True
    assert view["manager_present"] is True and view["view_target_is_character"] is True
    assert view["cache_time"] == 0
    assert view["returned_rotation_degrees"] == view["view_target_actor_rotation_degrees"]
    assert view["returned_rotation_degrees"]["yaw"] == 0
    assert row["control_rotation_degrees"]["yaw"] == 60


state = read("cold-state.json")
cold_contract(state)
for kind in range(4):
    broken = copy.deepcopy(state)
    view = broken["controller_snapshot"]["player_view"]
    if kind == 0:
        del broken["controller_snapshot"]["player_view"]
    elif kind == 1:
        view["manager_present"] = False
    elif kind == 2:
        view["cache_time"] = 1
    else:
        view["returned_rotation_degrees"]["yaw"] = 60
    try:
        cold_contract(broken)
    except (AssertionError, KeyError):
        continue
    raise AssertionError(kind)
a = read("baseline.json")
b = read("restored.json")
assert a == b and (r / "baseline.json").read_bytes() == (r / "restored.json").read_bytes()
assert read("ablated.report.json")["process_exit_code"] == 101
assert read("restored-build.report.json")["success"] is True
assert read("native.json.reader.report.json")["success"] is True
assert read("identity-check.json")["current_base_files_unchanged"] == 396
assert hashlib.sha256((r / "matrix.rs").read_bytes()).hexdigest() == read("final-source.json")["sha256"]
assert a["only_selected_angle_public_plan"][0] == a["native_actor_public_plan"][0]
assert a["current_public_plan"][0] != a["native_actor_public_plan"][0]
result = dict(gate="PASS_COLD_FALLBACK_SELECTOR_CONTRACT_AND_BYTE_RESTORATION", cold_negative_fixtures_rejected=4, source_actor_control_degrees=[0, 60], only_selected_angle_changed=True, source_bytes_restored=True, same_target_rebuilt=True, complete_counter_report_restored_byte_exact=True, native_input_event_executed=False, natural_nonzero_cold_input_claimed=False, full_world_feedback_claimed=False, known_angular_precision_residual_retained=a["remaining_angular_precision_residual"], new_difference="BF144-D272", screenshots=0, performance_acceptance="paused")
(r / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result), flush=True)
