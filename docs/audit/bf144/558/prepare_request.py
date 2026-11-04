import copy
import json
import math
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '557' / 'corner.request.json').read_bytes())
toward = next(case for case in request['cases'] if case['name'] == 'toward')
away = copy.deepcopy(toward)
away['name'] = 'away-first'
away['frames'][49]['movement_input_world'] = {'x': -.5, 'y': math.sqrt(.75), 'z': 0}
request['cases'] = [away, toward]
(root / 'away.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Prepared opposite first-wall acceptance for the second-impact input', flush=True)
