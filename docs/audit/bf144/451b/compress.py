import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
records={}
paths=[*sorted(r.glob('batch-*.native.json')),*sorted(r.glob('target-replay-*.json')),*sorted(r.glob('frame-table-*.json'))]
assert len(paths)==11
for p in paths:
    raw=p.read_bytes();print('Compressing complete Head matrix evidence '+p.name,flush=True)
    compressed=gzip.compress(raw,compresslevel=9,mtime=0);assert gzip.decompress(compressed)==raw
    q=p.with_suffix(p.suffix+'.gz');q.write_bytes(compressed)
    records[q.name]=dict(raw_bytes=len(raw),raw_sha256=hashlib.sha256(raw).hexdigest(),compressed_bytes=len(compressed),compressed_sha256=hashlib.sha256(compressed).hexdigest(),decompressed_exact=True)
    p.unlink()
(r/'compression-map.json').write_text(json.dumps(records,indent=2)+'\n');print('Verified all11 complete native/current matrices lossless and raw SHA exact',flush=True)
