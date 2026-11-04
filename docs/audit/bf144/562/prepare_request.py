import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '559' / 'third.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'stuck')
request['cases'] = []
for label, gap in (('blocked', .15), ('escape', .188)):
    case = copy.deepcopy(template)
    del case['corner_floor_top_cm']
    case.update(name=label, corner_slope_gap_cm=gap)
    for frame in case['frames']:
        frame['movement_input_world'] = {'x': 0, 'y': 0, 'z': 0}
    case['frames'][49]['falling_initial_cm'] = [200, 0, 200, 400, 400, 0]
    request['cases'].append(case)
(root / 'side.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared blocked and positive-travel lateral fallback cases', flush=True)
