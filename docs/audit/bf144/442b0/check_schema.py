import json
import sys
from pathlib import Path
r=Path(__file__).resolve().parent
sys.path.insert(0,str(Path('tools/tests').resolve()))
import test_movement_input_math_probe as schema
result=schema.self_test();v=json.loads((r/'before.native.json').read_text());failed=False
try:schema.validate(v)
except KeyError as e:failed=str(e)=="'move_input'"
assert failed
result['old_reader_missing_move_input_contract_rejected']=True
(r/'schema-check.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
