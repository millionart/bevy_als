import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '566' / 'bases.request.json').read_bytes())
template = request['cases'][1]
request['cases'] = []
for name, gap in (('overlap', 55), ('wide', 64), ('clear', 0)):
    case = copy.deepcopy(template)
    del case['walk_off_step_moves']
    case.update(name=name, floor_half_extent_x_cm=300, stuck_gap_cm=gap)
    case['frames'] = case['frames'][:42]
    case['frames'][40]['walking_initial_cm'][0] = 152
    case['frames'][40]['delta_seconds'] = .05
    request['cases'].append(case)
(root / 'recovery.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared recoverable overlap and ordinary Walking controls', flush=True)
