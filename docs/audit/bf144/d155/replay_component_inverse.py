import json
import math
from pathlib import Path


def yaw_rotate(vector, yaw):
    x, y, z = vector
    cosine, sine = math.cos(yaw), math.sin(yaw)
    return (cosine * x + sine * z, y, -sine * x + cosine * z)


def scale_by(vector, scale):
    return tuple(value * factor for value, factor in zip(vector, scale))


def source_inverse_position(point, translation, scale, yaw):
    if max(map(abs, scale)) <= 1e-8:
        return point
    reciprocal = tuple(1.0 / value if abs(value) > 1e-8 else 0.0 for value in scale)
    inverse_translation = yaw_rotate(scale_by(translation, reciprocal), -yaw)
    rotated = yaw_rotate(scale_by(point, reciprocal), -yaw)
    return tuple(value - offset for value, offset in zip(rotated, inverse_translation))


directory = Path(__file__).resolve().parent
source = json.loads((directory / "../d149/ab-als.native.json").read_text())
assert any('PropertyPath=("GetControlRigInput")' in prop["exported_text"]
           for obj in source["asset_objects"] for prop in obj["properties"])

translation = (2.0, 0.8, -1.0)
local = (0.196194, 0.134653, 0.073582)
nonuniform = (1.4, 0.8, 0.6)
rows = []
for name, scale, yaw in [
    ("unit", (1.0, 1.0, 1.0), 0.7),
    ("uniform", (1.5, 1.5, 1.5), 0.7),
    ("zero_yaw", nonuniform, 0.0),
    ("nonuniform_yaw", nonuniform, 0.7),
]:
    offset = yaw_rotate(scale_by(local, scale), yaw)
    point = tuple(value + origin for value, origin in zip(offset, translation))
    qst = source_inverse_position(point, translation, scale, yaw)
    affine = scale_by(yaw_rotate(offset, -yaw), tuple(1.0 / value for value in scale))
    rows.append({"case": name, "component_scale": scale, "yaw": yaw, "source": qst, "affine": affine,
                 "error_m": math.dist(qst, affine)})
assert [row["case"] for row in rows if row["error_m"] > 1e-6] == ["nonuniform_yaw"]
assert abs(rows[-1]["error_m"] - 0.170685) < 1e-6
assert source_inverse_position(local, translation, (0.0, 0.0, 0.0), 0.7) == local
print(json.dumps({"cases": rows, "all_zero_scale_returns_identity": True}, indent=2))
