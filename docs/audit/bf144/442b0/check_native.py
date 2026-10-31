import json
import sys
from pathlib import Path
r=Path(__file__).resolve().parent
sys.path.insert(0,str(Path('tools/tests').resolve()))
import test_movement_input_math_probe as schema
v=json.loads((r/'initial.native.json').read_text());result=schema.validate(v);rows=v['geometry_query_probe']['samples'];byname={x['input']['name']:x['move_input']for x in rows}
assert byname['forward']['world_input']=={'x':1,'y':0,'z':0}
assert byname['right']['world_input']=={'x':0,'y':1,'z':0}
assert byname['over']['clamped_x']==1 and byname['over']['clamped_y']==0
assert byname['zero']['world_input']=={'x':0,'y':0,'z':0}
assert byname['analog']['clamped_x']==.2 and byname['analog']['clamped_y']==.3
result.update(gate='PASS_NATIVE_SOURCE_MOVE_INPUT_STAGES',source_stage_values=byname)
(r/'native-check.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items()if k!='source_stage_values'}),flush=True)
