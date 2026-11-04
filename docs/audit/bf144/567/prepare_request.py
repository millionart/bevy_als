import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '566' / 'bases.request.json').read_bytes())
template = request['cases'][1]
request['cases'] = []
for name, moved in (('late', True), ('still', False)):
    case = copy.deepcopy(template)
    case.update(name=name, late_old_base_move=moved)
    case['frames'] = case['frames'][:43]
    request['cases'].append(case)
(root / 'late.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared matched old-base late transform controls', flush=True)
