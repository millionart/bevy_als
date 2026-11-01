import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent;mapping={}
for name in ['baseline.native.json','no-vm-transform.native.json','restored.native.json','default.native.json','character-default.native.json']:
 p=r/name;raw=p.read_bytes();packed=gzip.compress(raw,compresslevel=6,mtime=0);assert gzip.decompress(packed)==raw;dest=p.with_suffix('.json.gz');dest.write_bytes(packed);mapping[name]=dict(compressed=dest.name,raw_bytes=len(raw),raw_sha256=hashlib.sha256(raw).hexdigest(),compressed_bytes=len(packed),compressed_sha256=hashlib.sha256(packed).hexdigest());p.unlink();print('Compressed and verified original full native VM-space query and legacy evidence '+name,flush=True)
(r/'compression-map.json').write_text(json.dumps(mapping,indent=2)+'\n')
