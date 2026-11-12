import gzip
import json
import struct
from pathlib import Path


AUDIT = Path(__file__).resolve().parent


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def main():
    path = AUDIT / "weights.native.json"
    raw = path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix(".json.gz").read_bytes())
    native = json.loads(raw)
    machine = native["state_machine_players"]["machines"][0]
    assert machine["name"] == "Grounded States"
    cases = []
    native_frames = 0
    changed = 0
    largest = 0.0
    standalone = 0
    for sample in native["state_machine_entry_probe"]["samples"]:
        name = sample["name"]
        frames = sample["frames"]
        native_frames += len(frames)
        first = frames[0]["state"]
        second = frames[1]["state"]
        assert {first, second} == {1, 2}
        for index, frame in enumerate(frames[2:], 2):
            entries = frame["active_entries"]
            if len(entries) == 1 and name.endswith("_0.5"):
                entry = entries[0]
                expected = [entry["alpha"], f32(1.0 - entry["alpha"])]
                assert frame["updates"][first] == [expected[0]] and frame["updates"][second] == [expected[1]]
                standalone += 1
            if len(entries) != 2:
                continue
            old, current = entries
            assert [old["source"], old["target"], current["source"], current["target"]] == [first, second, second, first]
            old_alpha, current_alpha = old["alpha"], current["alpha"]
            remaining = f32(1.0 - current_alpha)
            first_old_path = f32(f32(1.0 - old_alpha) * remaining)
            actual = [f32(first_old_path + current_alpha), f32(old_alpha * remaining)]
            assert [frame["weights"][first], frame["weights"][second]] == actual
            assert frame["updates"][first] == [actual[0]] and frame["updates"][second] == [actual[1]]
            maximum = max(first_old_path, current_alpha)
            if maximum != actual[0]:
                changed += 1
                largest = max(largest, abs(maximum - actual[0]))
                cases.append({"name": f"{name}:{index}", "alphas": [old_alpha, current_alpha],
                              "weights": actual, "path_max": maximum})
    assert changed > 0 and standalone > 0
    print(json.dumps({"cases": cases, "native_frames": native_frames, "path_max_frames": changed,
                      "maximum_lost_weight": largest, "standalone_controls": standalone}, indent=2))


if __name__ == "__main__":
    main()
