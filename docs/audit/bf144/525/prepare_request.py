import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '524' / 'wall.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'positive')
request['cases'] = []
for name, observe in [('plane', True), ('plane-unobserved', False)]:
    case = copy.deepcopy(template)
    case.update(name=name, wall_pitch_degrees=-.03)
    if observe:
        for frame in (49, 50):
            case['frames'][frame]['observe_blocking_hits'] = True
    request['cases'].append(case)
(root / 'wall.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared identical above-threshold walls with the native observer enabled and disabled', flush=True)
