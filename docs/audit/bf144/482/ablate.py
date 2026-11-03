import gzip
import hashlib
import json
import sys
from pathlib import Path

root = Path(__file__).resolve().parent

def raw(name):
    path = root / name
    if path.exists():
        return path.read_bytes()
    return gzip.decompress((root / (name + '.gz')).read_bytes())

case = sys.argv[1]
canonical = raw('replay.rs.final')
inputs = raw('inputs.json.final')
if case == 'restore':
    (root/'replay.rs').write_bytes(canonical)
    (root/'inputs.json').write_bytes(inputs)
    print('Restored complete source and inputs byte for byte', flush=True)
    sys.exit(0)

data = json.loads(inputs)
source = [row for row in data['source'] if row['pc'] == 130 and row['dataset'] == 'weights' and row['weight'] == 0.25]
current = [row for row in data['current'] if row['pc'] == 130 and row['mode'] == 0 and row['query'] == 1]
assert len(source) == len(current) == 1
data.update(source=source, current=current)
selected = (json.dumps(data, separators=(',', ':'))+'\n').encode()
if case == 'setter':
    needle = b'                tree.set(foot, value);'
    assert canonical.count(needle) == 1
    variant = canonical.replace(needle, b'                let _ = value;')
elif case == 'local':
    begin = canonical.index(b'        fn final_local(')
    end = canonical.index(b'\n    }', begin)
    variant = canonical[:begin] + b'        fn final_local(&mut self, index: usize) -> RigTransform {\n            self.nodes[index].local\n        }' + canonical[end:]
else:
    raise AssertionError(case)
(root/('replay.no-'+case+'.rs')).write_bytes(variant)
(root/('negative-inputs-'+case+'.json')).write_bytes(selected)
(root/'replay.rs').write_bytes(variant)
(root/'inputs.json').write_bytes(selected)
(root/('negative-selection-'+case+'.json')).write_text(json.dumps(dict(case=case, source_sha256=hashlib.sha256(variant).hexdigest(), inputs_sha256=hashlib.sha256(selected).hexdigest()),indent=2)+'\n')
print(f'Prepared a native weighted frame and an actual public frame for {case}', flush=True)
