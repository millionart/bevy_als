import gzip
import hashlib
import json
from pathlib import Path

r = Path(__file__).resolve().parent
p = r / "source.native.json"
raw = p.read_bytes()
data = json.loads(raw)
case = data["character_animation_probe"]["cases"][0]
keys = ("frame", "world_seconds", "world_delta_seconds", "gravity_cm_s2", "actor_transform", "movement_before_tick", "movement_after_tick")
derived = dict(character_animation_probe=dict(cases=[dict(name=case["name"], frames=[{k:f[k] for k in keys} for f in case["frames"]])]))
(r / "source-phases.json").write_text(json.dumps(derived, indent=2) + "\n")
print("Extracted exact source movement stages", flush=True)
compressed = gzip.compress(raw, mtime=0)
assert gzip.decompress(compressed) == raw
target = p.with_suffix(p.suffix + ".gz")
target.write_bytes(compressed)
(r / "source-compression.json").write_text(json.dumps(dict(raw_sha256=hashlib.sha256(raw).hexdigest(), raw_bytes=len(raw), gzip_sha256=hashlib.sha256(compressed).hexdigest(), gzip_bytes=len(compressed), source_stage_projection_exact=True, raw_complete_animation_retained=True), indent=2) + "\n")
p.unlink()
print("Archived complete native report losslessly in Git evidence directory", flush=True)
