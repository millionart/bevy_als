import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
mapping={}
for p in sorted(r.glob('*.json')):
    selected={f'replay-{label}.json'for label in ['baseline','restored','no-delta-factor','no-product-factor','no-second-factor']}|{f'frame-table-{label}.json'for label in ['baseline','restored']}
    if p.name not in selected:continue
    data=p.read_bytes();packed=gzip.compress(data,mtime=0);assert gzip.decompress(packed)==data
    dest=p.with_suffix(p.suffix+'.gz');dest.write_bytes(packed)
    mapping[p.name]=dict(compressed=dest.name,raw_sha256=hashlib.sha256(data).hexdigest(),raw_bytes=len(data),compressed_sha256=hashlib.sha256(packed).hexdigest(),compressed_bytes=len(packed))
    p.unlink();print('Compressed and verified raw bytes '+p.name,flush=True)
assert len(mapping)==7
(r/'compression-map.json').write_text(json.dumps(mapping,indent=2)+'\n')
