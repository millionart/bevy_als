import gzip
import json
from pathlib import Path

source=json.loads(gzip.decompress(Path('docs/audit/bf144/473/weights.native.json.gz').read_bytes()))
item=source['rig_node_history_probe']['cases'][0]['frames'][1]['unit_boundaries'][0]['items'][0]
assert type(item.get('current_local_dirty_before_global_read'))is bool,'Native pre-output local dirty state is missing'
