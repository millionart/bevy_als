import json
from pathlib import Path

r=Path(__file__).resolve().parent;report=json.loads((r/'before.native.json').read_bytes());frame=report['rig_node_history_probe']['cases'][0]['frames'][1];assert 'unit_boundaries'in frame,'Exact native unit boundary data is missing'
