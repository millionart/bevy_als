import hashlib
import json
import sys
from pathlib import Path

r = Path(__file__).resolve().parent
changes = {
    "replay.rs": [("let predicted_position = start + delta;", "let predicted_position = (0..count).fold(start, |position, _| position + step);")],
    "playground_observed.rs": [("    app.add_systems(Update,c443_motor.after(bevy_tnua::TnuaPipelineSystems::Motors));\n", ""), ('"camera":camera,"position":position.0.to_array(),"velocity":velocity.0.to_array(),', '"camera":camera,')],
    "observer.rs": [('"camera":camera,"position":position.0.to_array(),"velocity":velocity.0.to_array(),', '"camera":camera,')],
}
if sys.argv[1] == "restore":
    for name, digest in json.loads((r / "final-sources.json").read_text()).items():
        raw = (r / (name + ".final")).read_bytes()
        assert hashlib.sha256(raw).hexdigest() == digest
        (r / name).write_bytes(raw)
else:
    digests = {}
    for name, edits in changes.items():
        p = r / name
        digests[name] = hashlib.sha256(p.read_bytes()).hexdigest()
        (r / (name + ".final")).write_bytes(p.read_bytes())
        s = p.read_text()
        for old, new in edits:
            assert s.count(old) == 1
            s = s.replace(old, new)
        p.write_text(s)
    (r / "final-sources.json").write_text(json.dumps(digests, indent=2) + "\n")
print("Physics evidence variant: " + sys.argv[1], flush=True)
