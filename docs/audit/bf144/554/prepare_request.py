import copy
import json
from pathlib import Path

root = Path(__file__).resolve().parent
request = json.loads((root.parent / '553' / 'lower.request.json').read_bytes())
case = next(case for case in request['cases'] if case['name'] == 'beyond')
request['cases'] = [case]
case['frames'].extend(copy.deepcopy(case['frames'][-1]) for _ in range(8))
assert len(case['frames']) == 51
(root / 'departure.request.json').write_text(json.dumps(request, separators=(',', ':')) + '\n')
print('Extended the unmodified low-deck native case through its far edge', flush=True)
