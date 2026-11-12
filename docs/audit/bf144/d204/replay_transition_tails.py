import gzip
import json
import struct
from pathlib import Path


ROOT = Path(__file__).parent


def read(name):
    path = ROOT / name
    if path.is_file():
        return json.loads(path.read_bytes())
    return json.loads(gzip.decompress(path.with_suffix(".json.gz").read_bytes()))


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def alpha(elapsed, duration):
    factor = f32(elapsed / duration)
    return f32(f32(factor * factor) * f32(3.0 - f32(2.0 * factor)))


def entry(source, target, elapsed, duration, inertial=False):
    return {"source": source, "target": target, "elapsed": elapsed,
            "duration": duration, "inertial": inertial}


def weights(frame, indices):
    return [next(iter(frame["updates"][index]), None) for index in indices]


def main():
    native = read("transitions.native.json")
    machine = native["state_machine_players"]["machines"][0]
    land, movement = machine["states"][3:5]
    first = next(edge for edge in land["exits"] if edge["index"] == 8)
    automatic = next(edge for edge in movement["exits"] if edge["index"] == 11)
    inertial = next(edge for edge in movement["exits"] if edge["index"] == 12)
    assert first["logic"] == automatic["logic"] == 0 and first["blend_mode"] == automatic["blend_mode"] == 2
    assert inertial["logic"] == 1
    duration = first["duration"]
    delta = f32(0.025)
    group = read("group.native.json")["sync_group_clock_probe"]
    exit_duration = f32(group["players"][1]["duration"] - 1.15625)
    cases = []
    failures = []
    for sample in native["state_machine_entry_probe"]["samples"]:
        name = sample["name"]
        mode, started = name.split("_")
        elapsed = f32(f32(float(started)) + delta)
        frame = sample["frames"][4]
        if mode == "uninterrupted" or frame["name"] == "Land Movement":
            continue
        assert frame["active_transitions"] == 0
        has_tail = elapsed < duration
        destination = 2 if mode == "inertial" else 0
        actual = weights(frame, [3, 4, destination])
        first_alpha = alpha(elapsed, duration) if has_tail else 1.0
        expected = ([f32(1.0 - first_alpha), first_alpha] if mode == "inertial" else [0.0, 0.0]) if has_tail else [None, None]
        assert actual == expected + [1.0], name
        assert [frame["weights"][index] for index in [3, 4, destination]] == [0.0, 0.0, 1.0]
        if actual != [None, None, 1.0]:
            failures.append(name)
        cases.append({"name": name, "entries": [entry(0, 1, elapsed, duration),
                      entry(1, 2, delta, 0.0 if mode == "inertial" else exit_duration, mode == "inertial")],
                      "updates": actual})

    standing = read("reentry.native.json")
    machine = standing["state_machine_players"]["machines"][1]
    assert machine["name"] == "Movement Details States"
    assert [machine["states"][index]["name"] for index in (5, 2, 0)] == ["Run Start", "Run", "Walk"]
    first = next(edge for edge in machine["states"][5]["exits"] if edge["index"] == 10)
    second = next(edge for edge in machine["states"][2]["exits"] if edge["index"] == 5)
    assert first["logic"] == second["logic"] == 0 and first["blend_mode"] == second["blend_mode"] == 2
    for sample in standing["state_machine_entry_probe"]["samples"]:
        name = sample["name"]
        frame = sample["frames"][4]
        elapsed = f32(f32(f32(float(name.split("_")[-1])) + delta) + delta)
        first_duration, second_duration = first["duration"], second["duration"]
        second_elapsed = f32(delta + delta)
        first_alpha = alpha(elapsed, first_duration) if elapsed < first_duration else 1.0
        second_alpha = alpha(second_elapsed, second_duration)
        expected = [f32(f32(1.0 - first_alpha) * f32(1.0 - second_alpha)) if elapsed < first_duration else 1.0,
                    f32(first_alpha * f32(1.0 - second_alpha)), second_alpha]
        actual = weights(frame, [5, 2, 0])
        assert actual == expected and all(len(frame["updates"][index]) == 1 for index in [5, 2, 0]), name
        assert frame["name"] == "Run Start" and frame["active_transitions"] == 0 and frame["weights"][5] == 1
        entries = [entry(0, 1, elapsed, first_duration)] if elapsed < first_duration else []
        entries += [entry(1, 2, second_elapsed, second_duration), entry(2, 0, delta, 0.0, True)]
        cases.append({"name": name, "entries": entries, "updates": actual})
        failures.append(name)

    clock_changes = {}
    samples = {sample["name"]: sample["frames"] for sample in group["samples"]}
    for name, source in samples.items():
        if not name.endswith("/source"):
            continue
        prefix = name.rsplit("/", 1)[0]
        early = samples[f"{prefix}/early_cleanup"]
        assert early[-2]["order"] == []
        actual = [trace[-1]["players"][0]["position"] for trace in (source, early)]
        if source[-2]["order"]:
            assert actual[0] == f32(source[-2]["players"][source[-2]["leader"]]["position"] + delta)
            assert actual[1] == delta
            clock_changes[prefix] = actual
        else:
            assert actual == [delta, delta]
    assert len(failures) == 8 and len(clock_changes) == 3
    result = {"cases": cases, "early_cleanup_ablation": failures, "returned_land_clock": clock_changes}
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
