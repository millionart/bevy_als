import gzip
import json
from pathlib import Path

r = Path(__file__).resolve().parent
with gzip.open("docs/audit/bf144/442c0/restored.native.json.gz", "rt") as stream:
    native = json.load(stream)
row = native["character_animation_probe"]["cases"][0]["frames"][0]["controller_before_tick"]
view = row["player_view"]
assert row["is_local_controller"] and view["manager_present"] and view["view_target_is_character"]
assert view["cache_time"] == 0
assert view["returned_rotation_degrees"] == view["view_target_actor_rotation_degrees"]
actor = view["returned_rotation_degrees"]["yaw"]
control = row["control_rotation_degrees"]["yaw"]
assert actor == 0 and control == 60
request = dict(samples=[dict(name=name, rotation_xyzw=[0, 0, 0, 1], move_input=dict(yaw_degrees=yaw, axes=[0, 1])) for name, yaw in (("cold-returned-actor", actor), ("cold-control", control))])
(r / "request.json").write_text(json.dumps(request, indent=2) + "\n")
(r / "cold-state.json").write_text(json.dumps(dict(source="docs/audit/bf144/442c0/restored.native.json.gz", controller_snapshot=row, held_input_axes=[0, 1], source_to_runtime_angle="phi = -theta - pi/2", natural_nonzero_input_claimed=False, enhanced_input_event_executed=False), indent=2) + "\n")
args = json.loads(Path("docs/audit/bf144/442b/matrix-args.json").read_text())
(r / "matrix-args.json").write_text(json.dumps([v.replace("/442b/", "/442c2/").replace("phase441c", "phase442c2") for v in args], indent=2) + "\n")
for name in ("run_matrix.ps1", "run_python.ps1"):
    (r / name).write_text(Path("docs/audit/bf144/442b", name).read_text().replace("/442b", "/442c2"))
print("Prepared two native math inputs from actual cold controller return and control fields", flush=True)
