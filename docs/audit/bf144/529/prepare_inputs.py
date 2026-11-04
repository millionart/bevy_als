import json
from pathlib import Path

root = Path(__file__).resolve().parent
native = json.loads((root.parent / '528' / 'summary.json').read_bytes())['native']
origins = {}
for label, record in native.items():
    point = record['first_hit']['move_stop_cm']
    origins[label] = [point['x']/100, 2+point['z']/100, point['y']/100]
(root / 'native-origins.json').write_text(json.dumps(origins, indent=2) + '\n')
print('Prepared native pulled-back wall positions in Playground coordinates', flush=True)
