import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '537' / 'wall.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'miss')
request['cases'] = []
for label, pitch, step in (('ramp-no-step', 40, False), ('ramp-step', 40, True), ('steep-no-step', 60, False)):
    case = copy.deepcopy(template)
    del case['step_fin']
    case.update(name=label, floor_half_extent_x_cm=200, ramp_pitch=pitch, ramp_step=step)
    request['cases'].append(case)
(root / 'ramp.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared shallow ramp permission pair and steep control', flush=True)
