import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
if sys.argv[1]=='restore':
    for name,digest in json.loads((r/'final-sources.json').read_text()).items():
        raw=(r/(name+'.final')).read_bytes()
        assert hashlib.sha256(raw).hexdigest()==digest
        (r/name).write_bytes(raw)
else:
    hashes={}
    for name in ['compare.py','replay.rs']:
        raw=(r/name).read_bytes()
        hashes[name]=hashlib.sha256(raw).hexdigest()
        (r/(name+'.final')).write_bytes(raw)
        text=raw.decode()
        if name=='compare.py':old='if abs(yaw-actual_control)>f32(1e-3):';new='if abs(yaw-actual_control)>0:'
        else:old='let direct=source+delta;';new='let direct=tick.advance_from_time(source).time;'
        assert text.count(old)==1
        (r/name).write_text(text.replace(old,new))
    (r/'final-sources.json').write_text(json.dumps(hashes,indent=2)+'\n')
print('Controller hold and direct-second factors: '+sys.argv[1],flush=True)
