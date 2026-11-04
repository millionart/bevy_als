import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '537' / 'wall.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'miss')
request['cases'] = []
for label, second in (('one-wall', False), ('two-walls', True)):
    case = copy.deepcopy(template)
    case.update(name=label, floor_half_extent_x_cm=200, step_corner_second=second)
    request['cases'].append(case)
(root / 'corner.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared equal StepUp inputs with only the second wall removed in the control', flush=True)
