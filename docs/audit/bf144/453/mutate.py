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
        'no-unit-stage':('modes[1].push(json!({"translation":narrow(early),"scale":narrow(direction(scale))}));','modes[1].push(json!({"translation":project(translation),"scale":narrow(direction(scale))}));'),
        'no-child-projection':('let current_t:V=std::array::from_fn(|k|(bf[k]as f32+df[k]as f32)as f64);','let current_t:V=narrow(early);'),
    }[mode]
    assert text.count(old)==1;p.write_text(text.replace(old,new))
print('Applied position-scale source control '+mode,flush=True)
