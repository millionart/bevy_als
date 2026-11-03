import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent;mapping={}
for p in sorted(r.glob('*.json'))+[r/'inputs.json.final']:
 if p.stat().st_size<4000000 and not p.name.startswith('replay-unit-'):continue
 print('Compressing local evidence '+p.name,flush=True);raw=p.read_bytes();packed=gzip.compress(raw,mtime=0);assert gzip.decompress(packed)==raw;target=p.with_suffix(p.suffix+'.gz');target.write_bytes(packed);mapping[p.name]=dict(raw_bytes=len(raw),raw_sha256=hashlib.sha256(raw).hexdigest(),compressed=target.name,compressed_sha256=hashlib.sha256(packed).hexdigest());p.unlink()
(r/'compression-map.json').write_text(json.dumps(mapping,indent=2)+'\n');print('Verified lossless local evidence '+str(len(mapping)),flush=True)
