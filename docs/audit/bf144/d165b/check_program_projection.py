import json
import struct
import subprocess
import sys
from pathlib import Path


def meter_float(centimeters):
    return struct.unpack("<f", struct.pack("<f", centimeters * 0.01))[0]


def project(value):
    if isinstance(value, list):
        return [project(child) for child in value]
    if not isinstance(value, dict):
        return value
    if value.keys() == {"Centimeters"}:
        values.append(value["Centimeters"])
        return {"F32": meter_float(value["Centimeters"])}
    return {key: project(child) for key, child in value.items()}


path = "assets/ALS/Character/CR_Als.program.json"
before = json.loads(subprocess.check_output(["git", "show", f"{sys.argv[1]}:{path}"]))
after = json.loads(Path(path).read_text())
values = []
assert project(after) == before
assert values and any(value != 0.0 for value in values)
print(json.dumps({"source_float_length_values": len(values), "other_program_fields_equal": True,
                  "old_meter_projection_reproduces_original_program": True}, indent=2))
