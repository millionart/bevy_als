import hashlib
import json
from pathlib import Path

r = Path(__file__).resolve().parent
depfile = next(Path("target/play/deps").glob("bevy_ecs-*.d"))
tokens = depfile.read_text().split()
facts = []
for suffix, ranges in (("change_detection/traits.rs", [(27, 30), (362, 368)]), ("change_detection/params.rs", [(15, 23)])):
    p = Path(next(v for v in tokens if v.endswith("/src/" + suffix)))
    lines = p.read_text().splitlines()
    facts.append(dict(key="dependency:bevy_ecs-0.19.1/src/" + suffix, sha256=hashlib.sha256(p.read_bytes()).hexdigest(), excerpts=[dict(start=a, end=b, text="\n".join(lines[a - 1:b])) for a, b in ranges]))
(r / "tick-source-facts.json").write_text(json.dumps(dict(facts=facts, conclusion="is_added is relative to the querying system last_run; later sampler eligibility cannot prove an earlier publisher observation", specific_missing_frame_root_cause_claimed=False), indent=2) + "\n")
print("Captured actual compiled dependency change-tick semantics", flush=True)
