import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
p=r/'replay.rs'
mode=sys.argv[1].split('-')[0]
assert mode in ['remove','restore']
if mode=='restore':
    raw=(r/'replay.rs.final').read_bytes()
    assert hashlib.sha256(raw).hexdigest()==json.loads((r/'final-sources.json').read_text())['replay.rs']
    p.write_bytes(raw)
else:
    raw=p.read_bytes()
    old='source_input.view_yaw_angle=f(&row["input"]["view_yaw_angle"]);'
    assert raw.decode().count(old)==1
    (r/'replay.rs.final').write_bytes(raw)
    (r/'final-sources.json').write_text(json.dumps({'replay.rs':hashlib.sha256(raw).hexdigest()},indent=2)+'\n')
    p.write_text(raw.decode().replace(old,''))
print('View yaw factor '+sys.argv[1],flush=True)
