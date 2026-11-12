import argparse
import json
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "d232c"))
from replay_layered_pose import ROOT, capture, read_capture, replay


def check(work):
    names = ("spine", "stop")
    for name in names:
        path = HERE / f"{name}.native.json"
        current = json.loads(path.read_bytes()) if path.exists() else read_capture(path.with_suffix(".json.gz"))
        assert current["layered_bone_blend_probe"] == capture(name)["layered_bone_blend_probe"]
    result = replay(work, names)
    assert result["source_bones"] == 948
    assert result["partial_weights"] > 0 and result["full_endpoint_projected_differences"] > 0
    return {**result, "result": "PASS_LAYERED_FULL_WEIGHT_COPY"}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--work", type=Path, default=ROOT / "target/d238")
    print(json.dumps(check(parser.parse_args().work), indent=2))
