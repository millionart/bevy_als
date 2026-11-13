import json
import struct
import subprocess
from pathlib import Path

old_revision = "f1529cba"
asset = "assets/ALS/Data/AnimationInstance/Grounded/als_standing_curves.json"
before = json.loads(subprocess.check_output(["git", "show", f"{old_revision}:{asset}"]))
after = json.loads(Path(asset).read_bytes())
row = next(row for row in json.loads(Path("docs/audit/bf144/u07/standing-yaw-curve/source-cases.json").read_bytes())
           if row["name"] == "actual-target-20")

def f32(value):
    return struct.unpack("f", struct.pack("f", value))[0]

def linear(curve, t):
    upper = next(i for i,time in enumerate(curve["times"]) if time>t)
    x,y=curve["times"][upper-1:upper+1]
    v,w=curve["values"][upper-1:upper+1]
    return f32(v+f32(f32(w-v)*f32(f32(t-x)/f32(y-x))))

angle = row["native"]["view_space_degrees"]
old = linear(before["yaw_forward_backward"], -angle)
new = -linear(after["yaw_forward_backward"], angle)
expected = -row["native"]["curve"]
assert old != expected and new == expected
print(json.dumps({"same_source_angle":angle,"reversed_key_runtime":old,
    "source_key_runtime":new,"native_runtime_projection":expected}))
