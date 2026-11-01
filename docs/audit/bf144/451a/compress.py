import gzip
import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
records={}
for label in ['baseline-fixed','ablated','restored']:
    p=r/('runtime-'+label)/'parameters.json';raw=p.read_bytes();print('Compressing full Head Apply evidence '+label,flush=True)
    compressed=gzip.compress(raw,compresslevel=9,mtime=0);assert gzip.decompress(compressed)==raw
    q=p.with_suffix('.json.gz');q.write_bytes(compressed)
    records[str(q.relative_to(r))]=dict(raw_bytes=len(raw),raw_sha256=hashlib.sha256(raw).hexdigest(),compressed_bytes=len(compressed),compressed_sha256=hashlib.sha256(compressed).hexdigest(),decompressed_exact=True)
    p.unlink()
(r/'compression-map.json').write_text(json.dumps(records,indent=2)+'\n')
print('Verified three lossless actual runtime histories and raw SHA identities',flush=True)
