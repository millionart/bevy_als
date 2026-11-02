import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent;mapping={}
for name in['rig.native.json','weights.native.json','replay-baseline.json','setter-matrix.json','setter-simplified.json','setter-restored.json','setter-no-weight.json']:
 p=r/name;raw=p.read_bytes();packed=gzip.compress(raw,mtime=0);assert gzip.decompress(packed)==raw;target=p.with_suffix(p.suffix+'.gz');target.write_bytes(packed);mapping[name]=dict(raw_bytes=len(raw),raw_sha256=hashlib.sha256(raw).hexdigest(),compressed=target.name,compressed_sha256=hashlib.sha256(packed).hexdigest());p.unlink();print('Preserved lossless evidence '+name,flush=True)
(r/'compression-map.json').write_text(json.dumps(mapping,indent=2)+'\n')
