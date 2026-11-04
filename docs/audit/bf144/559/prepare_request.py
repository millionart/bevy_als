import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '557' / 'corner.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'none')
request['cases'] = []
for label, top in (('stuck', 108.03), ('positive', 107.7)):
    case = copy.deepcopy(template)
    case.update(name=label, corner_floor_top_cm=top)
    case['frames'][49]['falling_initial_cm'] = [200, 0, 200, 400, 400, -100]
    request['cases'].append(case)
(root / 'third.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared two floor heights after perpendicular Air impacts', flush=True)
