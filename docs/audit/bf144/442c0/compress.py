import gzip
import hashlib
import json
from pathlib import Path

r = Path(__file__).resolve().parent
records = {}
for p in sorted(r.glob("*.native.json")):
    raw = p.read_bytes()
    compressed = gzip.compress(raw, mtime=0)
    assert gzip.decompress(compressed) == raw
    target = p.with_suffix(p.suffix + ".gz")
    target.write_bytes(compressed)
    records[p.name] = dict(raw_bytes=len(raw), raw_sha256=hashlib.sha256(raw).hexdigest(), gzip_bytes=len(compressed), gzip_sha256=hashlib.sha256(compressed).hexdigest())
    p.unlink()
    print("Compressed exact native output " + p.name, flush=True)
(r / "compression.json").write_text(json.dumps(records, indent=2) + "\n")
