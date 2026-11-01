import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
if sys.argv[1]=='restore':
    for name,digest in json.loads((r/'final-sources.json').read_text()).items():
        raw=(r/(name+'.final')).read_bytes();assert hashlib.sha256(raw).hexdigest()==digest;(r/name).write_bytes(raw)
else:
    hashes={}
    for name in ['observer.rs','playground_observed.rs','replay.rs']:
        p=r/name;raw=p.read_bytes();hashes[name]=hashlib.sha256(raw).hexdigest();(r/(name+'.final')).write_bytes(raw);s=raw.decode()
        if name=='replay.rs':
            old='let consumed_scale=extracted;';new='let consumed_scale=local_scale;'
        else:
            old=',"scale_inputs":scale_inputs';new=''
        assert s.count(old)==1;s=s.replace(old,new);p.write_text(s)
    (r/'final-sources.json').write_text(json.dumps(hashes,indent=2)+'\n')
print('Stored-scale evidence variant: '+sys.argv[1],flush=True)
