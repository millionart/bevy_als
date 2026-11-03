import gzip
import hashlib
import json
import sys
from pathlib import Path

root = Path(__file__).resolve().parent
def raw(name):
    path = root / name
    return path.read_bytes() if path.exists() else gzip.decompress((root/(name+'.gz')).read_bytes())

case = sys.argv[1]
canonical = raw('replay.rs.final')
inputs = raw('inputs.json.final')
if case == 'restore':
    (root/'replay.rs').write_bytes(canonical)
    (root/'inputs.json').write_bytes(inputs)
    print('Restored canonical source and inputs',flush=True)
    sys.exit(0)

data = json.loads(inputs)
data['source'] = [row for row in data['source'] if row['query'] == 1]
data['target'] = data['target'][:2]
assert len(data['source']) == 1 and [row['query'] for row in data['target']] == [0,1]
selected = (json.dumps(data,separators=(',',':'))+'\n').encode()
changes = {
    'native-final': (b'let normalized = native::normalized(q);',b'let normalized = q;'),
    'held-final': (b'let normalized = runtime_normalized(expected.rotation);',b'let normalized = expected.rotation;'),
    'prior-feedback': (b'let current: Option<&AlsRootAnimationContext> = previous.as_ref();',
                       b'let current: Option<&AlsRootAnimationContext> = previous.as_ref().map(|_| &projected);'),
}
old,new = changes[case]
assert canonical.count(old) == 1
variant = canonical.replace(old,new)
(root/'replay.rs').write_bytes(variant)
(root/'inputs.json').write_bytes(selected)
(root/f'replay.no-{case}.rs').write_bytes(variant)
(root/f'negative-inputs-{case}.json').write_bytes(selected)
sha = lambda value: hashlib.sha256(value).hexdigest()
(root/f'negative-selection-{case}.json').write_text(json.dumps(dict(case=case,source_sha256=sha(variant),inputs_sha256=sha(selected)),indent=2)+'\n')
print('Prepared one native and two public frames for '+case,flush=True)
