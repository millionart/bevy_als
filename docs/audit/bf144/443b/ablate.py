import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
digests={}
if sys.argv[1]=='restore':
    for name,digest in json.loads((r/'final-sources.json').read_text()).items():
        raw=(r/(name+'.final')).read_bytes();assert hashlib.sha256(raw).hexdigest()==digest;(r/name).write_bytes(raw)
else:
    for name in ['observer.rs','playground_observed.rs','replay.rs']:
        p=r/name;raw=p.read_bytes();digests[name]=hashlib.sha256(raw).hexdigest();(r/(name+'.final')).write_bytes(raw);s=raw.decode()
        if name=='replay.rs':
            old='let current=destination.y-q+(0.019f32+0.005f32);';new='let current=before.y;';assert s.count(old)==1;s=s.replace(old,new)
        else:
            old='"ordinal": probe.frame,"floor_probe":floor_probe,';assert s.count(old)==1;s=s.replace(old,'"ordinal": probe.frame,')
            if name=='playground_observed.rs':
                lines=s.splitlines(keepends=True);removed=[v for v in lines if 'app.add_systems('in v and ('c443b_raw_physics.'in v or 'c443b_coefficients.'in v)];assert len(removed)==2;s=''.join(v for v in lines if v not in removed)
        p.write_text(s)
    (r/'final-sources.json').write_text(json.dumps(digests,indent=2)+'\n')
print('Floor-band evidence variant: '+sys.argv[1],flush=True)
