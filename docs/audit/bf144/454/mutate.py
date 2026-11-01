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
        'no-product-order':('if !source_order {return a*b;}','if true {return a*b;}'),
        'no-reciprocal-method':('if !source_method {return q.normalize();}','if true {return q.normalize();}'),
    }[mode]
    assert text.count(old)==1;p.write_text(text.replace(old,new))
print('Applied rotation-stage source control '+mode,flush=True)
