import gzip
import json
import math
import struct
from pathlib import Path


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def heap_order(order, weights):
    def sift(root, size):
        while root < size // 2:
            child = 2 * root + 1
            right = child + 1
            if right < size and weights[order[right]] <= weights[order[child]]:
                child = right
            if weights[order[child]] >= weights[order[root]]:
                break
            order[root], order[child] = order[child], order[root]
            root = child

    for root in reversed(range(len(order) // 2)):
        sift(root, len(order))
    for end in reversed(range(1, len(order))):
        order[0], order[end] = order[end], order[0]
        sift(0, end)
    return order


def source_order(order, weights, depth, *, selection_only=False, allow_heap=True):
    count = len(order)
    if count < 2:
        return order
    if depth == 0 and allow_heap:
        return heap_order(order, weights)
    if count <= 8 or selection_only:
        for end in reversed(range(1, count)):
            last = min(range(end + 1), key=lambda index: weights[order[index]])
            order[last], order[end] = order[end], order[last]
        return order

    middle = count // 2
    order[0], order[middle] = order[middle], order[0]
    pivot = weights[order[0]]
    lower, upper = 1, count - 1
    while True:
        while lower < count and weights[order[lower]] >= pivot:
            lower += 1
        while upper > 0 and weights[order[upper]] <= pivot:
            upper -= 1
        if lower > upper:
            break
        order[lower], order[upper] = order[upper], order[lower]
        lower += 1
        upper -= 1
    order[0], order[upper] = order[upper], order[0]
    options = {"allow_heap": allow_heap}
    return (
        source_order(order[:upper], weights, depth - 1, **options)
        + order[upper:lower]
        + source_order(order[lower:], weights, depth - 1, **options)
    )


def main():
    root = Path(__file__).parent
    request = json.loads((root / "land.request.json").read_text())
    raw = root / "land.native.json"
    native_bytes = raw.read_bytes() if raw.exists() else gzip.decompress(raw.with_suffix(".json.gz").read_bytes())
    native = json.loads(native_bytes)["sync_group_clock_probe"]
    players = []
    for requested, recorded in zip(request["players"], native["players"], strict=True):
        assert requested["asset"] == recorded["asset_path"]
        assert not recorded.get("markers") and requested["start"] == 0 and not requested["looping"]
        players.append([recorded["duration"], f32(requested["rate"] * recorded["rate_scale"])])

    ablations = {"stable": [], "stable_leader": [], "selection_only": [], "no_heap": []}
    cases = []
    for requested, recorded in zip(request["samples"], native["samples"], strict=True):
        name = requested["name"]
        assert recorded["name"] == name
        positions = [0.0] * len(players)
        frames = []
        for frame_index, (step, frame) in enumerate(zip(requested["steps"], recorded["frames"], strict=True)):
            label = f"{name}:{frame_index}"
            weights = step["weights"]
            active = [index for index, weight in enumerate(weights) if weight is not None]
            depth = int(f32(math.log(len(active))) * 2)
            order = source_order(active.copy(), weights, depth)
            assert order == frame["order"], label
            leader = order[0]
            assert leader == frame["leader"], label
            stable = sorted(active, key=lambda index: -weights[index])
            if stable != order:
                ablations["stable"].append(label)
            if stable[0] != leader:
                ablations["stable_leader"].append(label)
            if source_order(active.copy(), weights, depth, selection_only=True) != order:
                ablations["selection_only"].append(label)
            if source_order(active.copy(), weights, depth, allow_heap=False) != order:
                ablations["no_heap"].append(label)

            delta = f32(step["delta_seconds"])
            duration, rate = players[leader]
            previous = positions[leader]
            current = min(duration, f32(previous + f32(rate * delta)))
            ratios = [f32(previous / duration), f32(current / duration)]
            assert ratios == [frame["previous_ratio"], frame["ratio"]], label
            for index in active:
                follower_duration = players[index][0]
                pair = [previous, current] if index == leader else [f32(ratio * follower_duration) for ratio in ratios]
                clock = frame["players"][index]
                assert pair == [clock["previous"], clock["position"]], (label, index)
                positions[index] = pair[1]
            frames.append({"delta": delta, "weights": weights, "order": order, "ratios": ratios,
                           "leader_clock": [previous, current], "positions": positions.copy()})
        cases.append({"name": name, "frames": frames})

    assert "land_handoff:1" in ablations["stable_leader"]
    assert ablations["selection_only"] and "partition_depth:0" in ablations["no_heap"]
    (root / "order.replay.json").write_text(json.dumps({"players": players, "cases": cases}, indent=2) + "\n")
    print(json.dumps({"cases": len(cases), "frames": sum(len(case["frames"]) for case in cases),
                      "ablations": ablations}, indent=2))


if __name__ == "__main__":
    main()
