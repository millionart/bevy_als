import hashlib
import json
import sys
from pathlib import Path

r = Path(__file__).resolve().parent
p = r / "matrix.rs"
original = "let selected_angle = raw_angle;"
replacement = "let selected_angle = source_angle;"
s = p.read_text()
if sys.argv[1] == "restore":
    assert s.count(replacement) == 1
    s = s.replace(replacement, original)
else:
    digest = hashlib.sha256(p.read_bytes()).hexdigest()
    (r / "final-source.json").write_text(json.dumps(dict(sha256=digest), indent=2) + "\n")
    assert s.count(original) == 1
    s = s.replace(original, replacement)
p.write_text(s)
if sys.argv[1] == "restore":
    assert hashlib.sha256(p.read_bytes()).hexdigest() == json.loads((r / "final-source.json").read_text())["sha256"]
print("Selected-angle variant: " + sys.argv[1], flush=True)
