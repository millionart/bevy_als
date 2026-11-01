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
        'no-delta-factor':('let d=if mode=="normalize-delta"{normalize(delta[i],false)}else{delta[i]};','let d=delta[i];'),
        'no-product-factor':('if mode=="normalize-product" {mesh[i]=normalize(mesh[i],false);}',''),
        'no-second-factor':('if mode!="one-local-normalize" {','{'),
        'no-sqrt-factor':('let std_sqrt=mode=="standard-sqrt";','let std_sqrt=false;'),
        'no-projection-factor':('let round=mode=="project-primitives";','let round=false;'),
        'native-no-neon':('let inv=if standard_sqrt {1.0/dot.sqrt()}else{unsafe {','let inv=if true {1.0/dot.sqrt()}else{unsafe {'),
    }[mode]
    assert text.count(old)==1;p.write_text(text.replace(old,new))
print('Applied source-arithmetic control '+mode,flush=True)
