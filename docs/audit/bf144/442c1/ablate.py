import hashlib
import json
import sys
from pathlib import Path

r = Path(__file__).resolve().parent
edits = [
    ("camera.is_active=!(20..22).contains(&probe.frame);", "camera.is_active=true;"),
    ("(c442c1_cold,c442c1_sample).chain().after", "c442c1_sample.after"),
]
digests = json.loads((r / "final-sources.json").read_text())
for name, digest in digests.items():
    p = r / name
    s = p.read_text()
    if sys.argv[1] == "restore":
        for original, replacement in edits:
            if replacement in s:
                assert s.count(replacement) == 1
                s = s.replace(replacement, original)
    else:
        assert hashlib.sha256(p.read_bytes()).hexdigest() == digest
        for original, replacement in edits:
            if original in s:
                assert s.count(original) == 1
                s = s.replace(original, replacement)
    p.write_text(s)
    if sys.argv[1] == "restore":
        assert hashlib.sha256(p.read_bytes()).hexdigest() == digest
print("Lifecycle observer variant: " + sys.argv[1], flush=True)
