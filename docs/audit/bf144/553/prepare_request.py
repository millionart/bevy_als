import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '550' / 'roof.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'clear')
request['cases'] = []
for label, depth in (('within', 44.7), ('beyond', 45.05)):
    case = copy.deepcopy(template)
    del case['step_roof_bottom_cm']
    case.update(name=label, floor_half_extent_x_cm=100, landing_deck_x_cm=190, landing_deck_top_cm=-depth)
    request['cases'].append(case)
(root / 'lower.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared both sides of the post-physics ground adjustment cap', flush=True)
