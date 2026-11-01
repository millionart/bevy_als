import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent;mapping={};paths=sorted(r.glob('runtime-*/parameters.json'))+sorted(r.glob('*.native.json'))
for p in paths:
 print('Compressing complete single-input audit evidence '+str(p.relative_to(r)),flush=True)
 raw=p.read_bytes();packed=gzip.compress(raw,compresslevel=6,mtime=0);assert gzip.decompress(packed)==raw
 dest=p.with_suffix('.json.gz');dest.write_bytes(packed);mapping[str(p.relative_to(r))]=dict(compressed=str(dest.relative_to(r)),raw_bytes=len(raw),raw_sha256=hashlib.sha256(raw).hexdigest(),compressed_bytes=len(packed),compressed_sha256=hashlib.sha256(packed).hexdigest());p.unlink()
assert len(mapping)==7
(r/'compression-map.json').write_text(json.dumps(mapping,indent=2)+'\n')
print('Verified seven original complete reports and rejected-order evidence without byte loss',flush=True)
