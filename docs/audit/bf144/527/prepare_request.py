import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '526' / 'wall.request.json').read_bytes())
template = next(case for case in request['cases'] if case['name'] == 'edge-in')
request['cases'] = []
for height in (19, 20.5, 21.5, 22.5, 23.5, 24.5):
    case = copy.deepcopy(template)
    case.update(name=f'top-{int(height*10)}', wall_top_cm=height)
    request['cases'].append(case)
(root / 'wall.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared six wall heights using the existing native edge witness', flush=True)
