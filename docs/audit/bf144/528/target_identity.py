import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
prior = json.loads((root.parent / '527' / 'target-binary.json').read_bytes())
path = Path(prior['path'])
with path.open('rb') as stream:
    digest = hashlib.file_digest(stream, 'sha256').hexdigest()
assert digest == prior['sha256'] and path.stat().st_size == prior['bytes']
print('Verified the unchanged edge observer binary', flush=True)
