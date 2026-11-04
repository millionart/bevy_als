import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '545' / 'overhang.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'open')
request['cases'] = []
for label, bottom in (('upper', 207), ('lower', 204)):
    case = copy.deepcopy(template)
    del case['step_overhang']
    case.update(name=label, step_ceiling_bottom_cm=bottom)
    request['cases'].append(case)
(root / 'edge.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared the same wall edge under two lift limits', flush=True)
