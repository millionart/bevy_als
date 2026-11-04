import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '537' / 'wall.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'miss')
request['cases'] = []
for label, center in (('tight', 91.8), ('open', 92.15)):
    case = copy.deepcopy(template)
    case.update(name=label, floor_half_extent_x_cm=200, step_overhang=True)
    case['frames'][40]['walking_initial_cm'][2] = center
    request['cases'].append(case)
(root / 'overhang.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared tight and ordinary Walking floor clearance under the same StepUp wall', flush=True)
