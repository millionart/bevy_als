import hashlib
import json
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
p=r/'compare.py'
if sys.argv[1]=='restore':
    raw=(r/'compare.py.final').read_bytes();assert hashlib.sha256(raw).hexdigest()==json.loads((r/'final-source.json').read_text())['sha256'];p.write_bytes(raw)
else:
    raw=p.read_bytes();old='source_rotation_to_runtime(q)';assert raw.decode().count(old)==1
    (r/'compare.py.final').write_bytes(raw);(r/'final-source.json').write_text(json.dumps(dict(sha256=hashlib.sha256(raw).hexdigest()),indent=2)+'\n')
    p.write_text(raw.decode().replace(old,'q'))
print('Quaternion coordinate basis '+sys.argv[1],flush=True)
