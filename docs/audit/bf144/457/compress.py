import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent;mapping={}
for p in sorted(r.glob('*.native.json')):
    raw=p.read_bytes();packed=gzip.compress(raw,compresslevel=6,mtime=0);assert gzip.decompress(packed)==raw
    dest=p.with_suffix('.json.gz');dest.write_bytes(packed);mapping[p.name]=dict(compressed=dest.name,raw_bytes=len(raw),raw_sha256=hashlib.sha256(raw).hexdigest(),compressed_bytes=len(packed),compressed_sha256=hashlib.sha256(packed).hexdigest());p.unlink();print('Compressed and verified complete native bytes '+p.name,flush=True)
assert len(mapping)==6
(r/'compression-map.json').write_text(json.dumps(mapping,indent=2)+'\n')
