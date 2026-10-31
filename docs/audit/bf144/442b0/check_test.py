import json
import sys
from pathlib import Path
r=Path(__file__).resolve().parent
sys.path.insert(0,str(Path('tools/tests').resolve()))
import test_movement_input_math_probe as test
result=test.self_test();result['restored_native_contract']=test.validate(json.loads((r/'restored.native.json').read_text()))
(r/'test-check.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result),flush=True)
