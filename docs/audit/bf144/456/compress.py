import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent;mapping={}
for p in sorted(r.glob('runtime-*/parameters.json')):
    print('Compressing original complete VM parameters '+p.parent.name,flush=True)
    data=p.read_bytes();packed=gzip.compress(data,compresslevel=6,mtime=0);assert gzip.decompress(packed)==data
    dest=p.with_suffix('.json.gz');dest.write_bytes(packed)
    mapping[str(p.relative_to(r))]=dict(compressed=str(dest.relative_to(r)),raw_bytes=len(data),raw_sha256=hashlib.sha256(data).hexdigest(),compressed_bytes=len(packed),compressed_sha256=hashlib.sha256(packed).hexdigest())
    p.unlink();print('Verified complete original VM raw bytes '+p.parent.name,flush=True)
assert len(mapping)==7
(r/'compression-map.json').write_text(json.dumps(mapping,indent=2)+'\n')
