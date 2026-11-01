import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
p=r/'replay.rs';saved=r/'replay.rs.final';mode=sys.argv[1]
if mode=='save':
    assert not saved.exists();saved.write_bytes(p.read_bytes())
    (r/'final-source.json').write_text(json.dumps(dict(sha256=hashlib.sha256(p.read_bytes()).hexdigest()),indent=2)+'\n')
elif mode=='restore':
    p.write_bytes(saved.read_bytes());assert p.read_bytes()==saved.read_bytes()
else:
    text=saved.read_text()
    old,new={
        'no-delta-factor':('let d=if delta_n {d.normalize()} else {d};','let d=d.normalize();'),
        'no-product-factor':('mesh.insert(*id,if product_n {q.normalize()}else{q});','mesh.insert(*id,q.normalize());'),
        'no-second-factor':('local.insert(*id,if second_local_n {q.normalize()}else{q});','local.insert(*id,q);'),
    }[mode]
    assert text.count(old)==1;p.write_text(text.replace(old,new))
print('Applied rotation-stage source control '+mode,flush=True)
