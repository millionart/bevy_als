import gzip
import json
import struct
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def load(path):
    data = path.read_bytes() if path.is_file() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    return json.loads(data)


directory = Path(__file__).resolve().parent
source = load(directory / "locomotion.native.json")
event_graph = next(graph for graph in source["graphs"] if graph["name"] == "EventGraph")
node = next(node for node in event_graph["nodes"] if node["node_guid"] == "183f8499-4c3b-6ddd-3000-fe957622821c")
pins = {pin["name"]: pin for pin in node["pins"]}
assert node["title"].startswith("Play Transition Right Animation")
predecessors = {link["node_object_path"] for link in pins["execute"]["linked_to"]}
assert any("LandToGrounded" in event["title"] and event["object_path"] in predecessors for event in event_graph["nodes"])
profile = {"blend_in": float(pins["BlendInDuration"]["default_value"]),
           "blend_out": float(pins["BlendOutDuration"]["default_value"]),
           "play_rate": float(pins["PlayRate"]["default_value"]),
           "start": float(pins["StartTime"]["default_value"])}
assert profile == {"blend_in": 0.1, "blend_out": 0.2, "play_rate": 1.4, "start": 0.0}
assert pins["bFromStandingIdleOnly"]["default_value"].lower() == "false"
request = load(directory / "landing.request.json")
assert request["dynamic_montage"]["blend_in_seconds"] == profile["blend_in"]
assert request["dynamic_montage"]["blend_out_seconds"] == profile["blend_out"]
assert all(sample["play_rate"] == profile["play_rate"] for sample in request["samples"])

cases = []
for name, sequence in (("standing", "A_Als_Stand_Transition_Right"), ("crouching", "A_Als_Crouch_Transition_Right")):
    native = load(directory / f"{name}.native.json")
    assert native["asset_path"].endswith(f"/{sequence}.{sequence}")
    probe = native["montage_clock_probe"]
    assert [sample["name"] for sample in probe["samples"]] == ["landing", "replaced"]
    entry, replaced = (sample["frames"] for sample in probe["samples"])
    assert [frame["weight"] for frame in entry] == [0.5, 1.0, 1.0]
    assert [frame["weight"] for frame in replaced] == [1.0, 0.5, 0.0]
    assert entry[0]["position"] == f32(f32(profile["play_rate"]) * f32(0.05))
    cases.append({"crouching": name == "crouching", "sequence": sequence, "duration": probe["montage_length"],
                  "first_position": entry[0]["position"], "first_weight": entry[0]["weight"],
                  "replaced_weight": replaced[1]["weight"]})

old_position = f32(f32(0.2) + f32(f32(1.5) * f32(0.05)))
assert old_position != cases[0]["first_position"] and 0.15625 != cases[0]["first_weight"]
assert 0.84375 != cases[0]["replaced_weight"]
print(json.dumps({"profile": profile, "cases": cases, "old_first_position": old_position,
                  "old_first_weight": 0.15625, "old_replaced_weight": 0.84375}, indent=2))
