import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '525' / 'wall.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'plane')
request['cases'] = []
for name, top, deck in [('edge-in', 16, -2.7), ('edge-out', 29, -2.7), ('edge-gap', 16, -6.5), ('edge-in-unobserved', 16, -2.7)]:
    case = copy.deepcopy(template)
    case.update(name=name, wall_pitch_degrees=0, wall_top_cm=top, landing_deck_top_cm=deck)
    if name.endswith('unobserved'):
        for frame in (49, 50):
            del case['frames'][frame]['observe_blocking_hits']
    request['cases'].append(case)
(root / 'wall.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared native inside, outside and deeper-floor edge witnesses', flush=True)
