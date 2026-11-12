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
    native = json.loads(raw)["blend_list_probe"]["samples"]
    request = json.loads((AUDIT / "weights.request.json").read_bytes())["samples"]
    fixture = []
    differences = 0
    cached_differences = 0
    frames = 0
    for requested, observed in zip(request, native, strict=True):
        assert requested["name"] == observed["name"]
        for index, (step, frame) in enumerate(zip(requested["steps"], observed["frames"], strict=True)):
            frames += 1
            root = f32(step["parent_weight"])
            outer = frame["outer_alpha"]
            details = f32(1.0 - outer)
            inner = frame["two_way"]["alpha"]
            state_weight = f32(root * details)
            sprint = frame["weights"][1]
            child_weight = f32(state_weight * sprint)
            for factor, observed_weights in zip((f32(1.0 - inner), inner), frame["two_way"]["updates"], strict=True):
                expected = f32(child_weight * factor)
                assert observed_weights == [expected], (requested["name"], index)
                old = f32(root * f32(details * f32(sprint * factor)))
                differences += expected != old
            base = frame["weights"][0]
            direct_request = f32(root * outer)
            requests = [f32(state_weight * base), direct_request] if base > f32(1e-5) else [direct_request]
            assert requests == frame["cached_requests"], (requested["name"], index)
            assert frame["cached_updates"] == [max(requests)], (requested["name"], index)
            old_cached = max(direct_request, f32(root * f32(details * base))) if base > f32(1e-5) else direct_request
            cached_differences += old_cached != max(requests)
            if index == 0:
                assert sprint == 1.0
                leaf = frame["two_way"]["updates"][1][0]
                fixture.append({"name": requested["name"], "root": root, "outer": outer, "inner": inner,
                                "leaf": leaf, "cached": max(direct_request, leaf)})
    first = fixture[0]
    wrong = f32(first["root"] * f32(f32(1.0 - first["outer"]) * first["inner"]))
    assert [first["leaf"], wrong] == [0.05908052623271942, 0.05908053368330002]
    assert differences > 0
    print(json.dumps({"frames": fixture, "native_frames": frames, "reverse_product_differences": differences,
                      "cached_max_differences": cached_differences, "first_ablation": [wrong, first["leaf"]]}, indent=2))


if __name__ == "__main__":
    main()
