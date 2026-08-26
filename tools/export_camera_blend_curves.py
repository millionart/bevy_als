"""Sample CF_Als_CameraBlend_Quick/Smooth via UE ProgrammaticToolset Python (rule 5 export)."""
import json
import subprocess
import sys

PROBE = r"D:/Users/milli/Git/bevy_als/tools/mcp_probe.py"

SCRIPT = r'''
import unreal
out = {}
for name in ["CF_Als_CameraBlend_Quick", "CF_Als_CameraBlend_Smooth"]:
    path = f"/ALS/ALSCamera/Data/{name}"
    curve = unreal.load_asset(path, unreal.CurveFloat)
    if curve is None:
        out[name] = {"error": "load failed"}
        continue
    times = unreal.Array(unreal.Float)
    values = unreal.Array(unreal.Float)
    unreal.CurveFloat.get_keys(curve, times, values) if hasattr(unreal.CurveFloat, "get_keys") else None
    # Fallback: iterate keys via rich curve API
    keys = []
    try:
        rich = curve.get_editor_property("float_curve")
    except Exception:
        rich = None
    if rich is not None:
        n = unreal.RichCurve.get_number_of_keys(rich)
        for i in range(n):
            k = unreal.RichCurve.get_key_at_index(rich, i)
            keys.append({
                "t": unreal.RichCurveKey.get_time(k),
                "v": unreal.RichCurveKey.get_value(k),
                "interp": str(unreal.RichCurveKey.get_interp_mode(k)),
            })
    out[name] = {"n": len(keys), "keys": keys}
print("CURVE_EXPORT_BEGIN")
print(json.dumps(out))
print("CURVE_EXPORT_END")
'''


def main() -> None:
    r = subprocess.run(
        [sys.executable, PROBE, "call",
         "editor_toolset.toolsets.programmatic.ProgrammaticToolset",
         "execute_tool_script", json.dumps({"script": SCRIPT})],
        capture_output=True, text=True, timeout=600,
    )
    out = r.stdout
    begin = out.find("CURVE_EXPORT_BEGIN")
    end = out.find("CURVE_EXPORT_END")
    if begin == -1 or end == -1:
        print("no markers; raw head:", out[:1500])
        return
    payload = json.loads(out[begin + len("CURVE_EXPORT_BEGIN"):end].strip())
    with open(r"D:/Users/milli/Git/bevy_als/docs/bf9-camera-blend-curves.json", "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=1)
    for name, data in payload.items():
        if "keys" in data:
            print(name, "keys:", data["n"], "first:", data["keys"][0], "last:", data["keys"][-1])
        else:
            print(name, data)


if __name__ == "__main__":
    main()
