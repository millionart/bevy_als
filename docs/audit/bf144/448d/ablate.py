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
    for name in ['observer.rs','playground_observed.rs','scene_observer.rs']:
        raw=(r/name).read_bytes();hashes[name]=hashlib.sha256(raw).hexdigest();(r/(name+'.final')).write_bytes(raw)
        if name!='scene_observer.rs':
            text=raw.decode();old='"scene_root":c448d_scene_root(player,instance.player_entity(),&skeletons,&scene_targets,&transforms),';assert text.count(old)==1;(r/name).write_text(text.replace(old,''))
    (r/'final-sources.json').write_text(json.dumps(hashes,indent=2)+'\n')
print('Actual scene target observation '+sys.argv[1],flush=True)
