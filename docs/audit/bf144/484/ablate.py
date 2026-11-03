import gzip
import hashlib
import json
import sys
from pathlib import Path

root = Path(__file__).resolve().parent
def raw(name):
    path = root/name
    return path.read_bytes() if path.exists() else gzip.decompress((root/(name+'.gz')).read_bytes())

case = sys.argv[1]
source = raw('replay.rs.final')
inputs = raw('inputs.json.final')
if case == 'restore':
    (root/'replay.rs').write_bytes(source)
    (root/'inputs.json').write_bytes(inputs)
    print('Restored canonical source and inputs',flush=True)
    sys.exit(0)
data = json.loads(inputs)
row = next(row for row in data['rows'] if row['query'] == 100)
if case == 'normalization':
    needle = b'if corrected && let Some(rotation)'
    assert source.count(needle) == 1
    source = source.replace(needle,b'if false && corrected && let Some(rotation)')
elif case == 'current-root':
    next_row = next(row for row in data['rows'] if row['query'] == 101)
    assert row['prior_root'] == 99 and next_row['prior_root'] == 100
    row['locals'] = next_row['locals']
else:
    raise AssertionError(case)
data['rows'] = [row]
selected = (json.dumps(data,separators=(',',':'))+'\n').encode()
(root/'replay.rs').write_bytes(source)
(root/'inputs.json').write_bytes(selected)
(root/f'replay.no-{case}.rs').write_bytes(source)
(root/f'negative-inputs-{case}.json').write_bytes(selected)
sha = lambda value: hashlib.sha256(value).hexdigest()
(root/f'negative-selection-{case}.json').write_text(json.dumps(dict(case=case,source_sha256=sha(source),inputs_sha256=sha(selected)),indent=2)+'\n')
print('Prepared frame 100 control '+case,flush=True)
