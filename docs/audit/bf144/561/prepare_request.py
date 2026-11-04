import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '557' / 'corner.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'none')
request['cases'] = []
for label, top in (('third', 104), ('second', 106.5)):
    case = copy.deepcopy(template)
    case.update(name=label, third_floor_top_cm=top)
    for frame in case['frames']:
        frame['movement_input_world'] = {'x': 0, 'y': 0, 'z': 0}
    request['cases'].append(case)
(root / 'time.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared ordinary second and third-impact landing cases', flush=True)
