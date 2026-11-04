import hashlib
import json
import sys
from pathlib import Path

root = Path(__file__).resolve().parent
path = Path('tools/tmp/bf144/526/playground_observed')
with path.open('rb') as stream:
    digest = hashlib.file_digest(stream, 'sha256').hexdigest()
identity = dict(path=str(path), sha256=digest, bytes=path.stat().st_size)
record = root / 'target-binary.json'
if sys.argv[1] == 'before':
    record.write_text(json.dumps(identity, indent=2) + '\n')
else:
    assert json.loads(record.read_bytes()) == identity
print('Verified reused edge observer binary identity', flush=True)
