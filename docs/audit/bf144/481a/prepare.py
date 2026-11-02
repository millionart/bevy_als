import gzip
import json
from pathlib import Path

r=Path(__file__).resolve().parent
for name in ['actual-before','actual-after','weights-before','weights-after']:
    p=Path('docs/audit/bf144/476')/(name+'.request.json')
    raw=p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix('.json.gz').read_bytes())
    request=json.loads(raw)
    assert request['capture_current_hierarchy']and request['capture_float_spring_state']
    assert request['unit_boundary']['function']in ['FAlsRigUnit_CalculatePoleVector::Execute','FRigUnit_TwoBoneIKSimplePerItem::Execute']
    (r/(name+'.request.json')).write_bytes(raw)
print('Copied four complete native pre/post histories without changing any request bytes or original source record',flush=True)
