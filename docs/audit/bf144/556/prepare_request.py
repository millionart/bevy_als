import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '555' / 'apex.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'clear')
request['cases'] = []
for label, vx, vy in (('axis-low', .02, 0), ('diagonal-low', .022, .022),
                      ('diagonal-high', .023, .023), ('zero', 0, 0)):
    case = copy.deepcopy(template)
    case.update(name=label)
    case['frames'][49].update(delta_seconds=.06, falling_initial_cm=[200, 0, 200, vx, vy, -20])
    request['cases'].append(case)
(root / 'residual.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared a two-substep fall with axial and diagonal tiny velocities', flush=True)
