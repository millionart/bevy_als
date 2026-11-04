import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '523' / 'wall.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'wall-only')
request['cases'] = []
for label, split, roof in (('ceiling-split', True, True), ('ceiling-unsplit', False, True), ('clear', True, False)):
    case = copy.deepcopy(template)
    del case['wall_x_cm']
    case.update(name=label, force_apex_split=split, apex_roof=roof, floor_half_extent_x_cm=200)
    case['frames'][49].update(delta_seconds=.05, falling_initial_cm=[200, 0, 200, 0, 0, 20],
                              movement_input_world={'x': 0, 'y': 0, 'z': 0}, observe_blocking_hits=True)
    request['cases'].append(case)
(root / 'apex.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
initial = dict(position_m=[-13, 4, -15], velocity_mps=[0, .2, 0], delta_s=.05)
(root / 'initial.json').write_text(json.dumps(initial, indent=2) + '\n')
print('Prepared native apex split and no-split cases under the same roof', flush=True)
