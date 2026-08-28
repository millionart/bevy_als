import json
import os

import unreal


root = os.environ.get("BEVY_ALS_ROOT")
if root is None:
    raise RuntimeError("BEVY_ALS_ROOT is required")

names = [
    "CV_Als_AccelerationAndDecelerationAndGroundFriction_Normal",
    "CV_Als_AccelerationAndDecelerationAndGroundFriction_Responsive",
    "CV_Als_AccelerationAndDecelerationAndGroundFriction_Sluggish",
]
sample_times = [i / 20.0 for i in range(61)]
result = {}

for name in names:
    path = "/ALS/ALS/Data/Character/Movement/" + name
    curve = unreal.load_asset(path, unreal.CurveVector)
    if curve is None:
        raise RuntimeError("failed to load " + path)
    samples = []
    for time in sample_times:
        value = curve.get_vector_value(time)
        samples.append({"time": time, "x": value.x, "y": value.y, "z": value.z})
    result[name] = {"asset": path, "samples": samples}

out_dir = os.path.join(root, "tmp", "export", "ground_dynamics_curves")
os.makedirs(out_dir, exist_ok=True)
out_path = os.path.join(out_dir, "curves.json")
with open(out_path, "w", encoding="utf-8") as stream:
    json.dump(result, stream, indent=1)
print("PYEXPORT " + json.dumps({"out": out_path, "assets": len(result)}))
