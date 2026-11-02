import gzip
import json
from pathlib import Path

old=json.loads(gzip.decompress(Path('docs/audit/bf144/476/weights-after.native.json.gz').read_bytes()))
boundary=next(row for case in old['rig_node_history_probe']['cases']for frame in case['frames']for row in frame['unit_boundaries'])
assert 'hierarchy_cache_before_item_reads'in boundary,'Old native report lacks raw hierarchy cache state; descendant propagation is unqualified'
