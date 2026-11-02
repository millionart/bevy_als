import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent;mapping={}
for p in sorted(r.glob('*.json')):
 if p.name=='inputs.json'or p.stat().st_size<2000000:continue
 print('Compressing unit phase evidence '+p.name,flush=True);raw=p.read_bytes();packed=gzip.compress(raw,mtime=0);assert gzip.decompress(packed)==raw;p.with_suffix(p.suffix+'.gz').write_bytes(packed);mapping[p.name]=dict(raw_bytes=len(raw),raw_sha256=hashlib.sha256(raw).hexdigest(),compressed=p.name+'.gz',compressed_sha256=hashlib.sha256(packed).hexdigest());p.unlink()
(r/'compression-map.json').write_text(json.dumps(mapping,indent=2)+'\n');print('Verified original unit phase evidence bytes '+str(len(mapping)),flush=True)
