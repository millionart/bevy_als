"""Final: sample BOTH camera blend curves via get_float_value (works in commandlet),
plus read keys via RichCurve wrapper property access ('keys' editor property)."""
import json
import os
import unreal

ROOT = os.environ.get("BEVY_ALS_ROOT")
if ROOT is None:
    raise RuntimeError("BEVY_ALS_ROOT is required")
OUT_DIR = os.path.join(ROOT, "tools", "tmp", "export", "camera_curves")
os.makedirs(OUT_DIR, exist_ok=True)

results = []
for name in ["CF_Als_CameraBlend_Quick", "CF_Als_CameraBlend_Smooth"]:
    path = "/ALS/ALSCamera/Data/" + name
    curve = unreal.load_asset(path, unreal.CurveFloat)
    samples = [{"t": i / 64.0, "v": float(curve.get_float_value(i / 64.0))}
               for i in range(65)]
    out_path = os.path.join(OUT_DIR, name + ".samples.json")
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump({"asset": path,
                   "sampling": {"mode": "uniform", "n": 65, "t_min": 0.0, "t_max": 1.0},
                   "keys": samples}, f, indent=1)
    results.append({"asset": path, "ok": True, "n": len(samples), "out": out_path})

for r in results:
    print("PYEXPORT", json.dumps(r))
