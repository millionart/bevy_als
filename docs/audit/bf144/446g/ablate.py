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
    for name in ['observer.rs','playground_observed.rs','replay.rs']:
        raw=(r/name).read_bytes();hashes[name]=hashlib.sha256(raw).hexdigest();(r/(name+'.final')).write_bytes(raw)
        text=raw.decode()
        if name=='replay.rs':old='let control_spring=0.0;';new='let control_spring=spring;'
        else:old='"physics_motors":probe.physics_motor_rows,"physics_coefficients":probe.physics_coefficient_rows,"physics_raw":probe.physics_raw_rows,';new=''
        assert text.count(old)==1
        (r/name).write_text(text.replace(old,new))
    (r/'final-sources.json').write_text(json.dumps(hashes,indent=2)+'\n')
print('Cold physics observation and spring-only factor: '+sys.argv[1],flush=True)
