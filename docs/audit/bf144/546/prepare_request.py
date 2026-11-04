import copy
import json
import math
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '545' / 'overhang.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'tight')
request['cases'] = []
for label, pitch in (('opposed', 4), ('following', -4)):
    case = copy.deepcopy(template)
    case.update(name=label, step_floor_pitch=pitch)
    case['frames'][40]['walking_initial_cm'][2] = 61.8 + 30 / math.cos(math.radians(pitch))
    request['cases'].append(case)
(root / 'floor.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared equal tight floor clearance with opposite slope signs', flush=True)
