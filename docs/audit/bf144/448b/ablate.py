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
    for name in ['observer.rs','playground_observed.rs','pose_observer.rs']:
        raw=(r/name).read_bytes();hashes[name]=hashlib.sha256(raw).hexdigest();(r/(name+'.final')).write_bytes(raw)
        if name!='pose_observer.rs':
            text=raw.decode();old='"pose_boundaries":c448b_pose_boundaries(player,&graphs,&skeletons,&assets),';assert text.count(old)==1;(r/name).write_text(text.replace(old,''))
    (r/'final-sources.json').write_text(json.dumps(hashes,indent=2)+'\n')
print('Actual Pose observation '+sys.argv[1],flush=True)
