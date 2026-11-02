import json
from pathlib import Path

r=Path(__file__).resolve().parent;proof=json.loads((r/'lifetime-proof.json').read_bytes());assert proof['actual_calls']==636 and proof['controlled_calls']==48 and proof['source_pin_values']==9576
assert proof['foot_current_global_not_qualified']is False,'Frame-end foot rotation is overwritten after the consumer and cannot qualify the exact input'
