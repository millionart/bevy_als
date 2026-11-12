import gzip
import importlib.util
import json
from pathlib import Path


AUDIT = Path(__file__).resolve().parent


def main():
    spec = importlib.util.spec_from_file_location("source_blend_list", AUDIT.parent / "d214/replay_blend_list_clock.py")
    blend = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(blend)
    path = AUDIT / "relevancy.native.json"
    raw = path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    native = json.loads(raw)
    node = next(node for graph in native["graphs"] for node in graph["nodes"]
                if node["node_guid"] == "d68d1f35-4127-d675-2dfd-8697d49c44ad")
    fields = next(prop for prop in node["properties"] if prop["name"] == "Node")["fields"]
    assert next(field for field in fields if field["name"] == "ChildUpateMode")["exported_text"] == "Default"
    assert next(field for field in fields if field["name"] == "BlendType")["exported_text"] == "Cubic"
    request = json.loads((AUDIT / "relevancy.request.json").read_bytes())
    cases = []
    extra = []
    for requested, observed in zip(request["samples"], native["blend_list_probe"]["samples"], strict=True):
        name = requested["name"]
        assert name == observed["name"]
        weights = blend.simulate(requested["steps"], list(map(blend.f32, request["blend_times"])), request["mode"])
        active = None
        frames = []
        for index, (expected, frame) in enumerate(zip(weights, observed["frames"], strict=True)):
            assert frame["weights"] == expected["weights"] and frame["duration"] == expected["duration"]
            forced = active if active is not None and active != frame["active"] and frame["duration"] == 0.0 else None
            active = frame["active"]
            updates = [[0.0] if forced == child else [] for child in range(2)]
            for child, weight in enumerate(frame["weights"]):
                if weight > blend.f32(1e-5):
                    updates[child].append(weight)
                if blend.f32(1e-6) < weight <= blend.f32(1e-5):
                    assert forced != child
                    extra.append([name, index, child, weight])
            assert updates == frame["updates"], (name, index)
            assert frame["cached_updates"] == [max(updates[0])], (name, index)
            frames.append({"active": active, "delta": expected["delta"], "updated": list(map(bool, updates))})
        cases.append({"name": name, "frames": frames})
    assert len(extra) == 7 and all(row[2] == 1 for row in extra)
    print(json.dumps({"cases": cases, "native_frames": sum(len(case["frames"]) for case in cases),
                      "old_threshold_extra_updates": extra}, indent=2))


if __name__ == "__main__":
    main()
