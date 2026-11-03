import copy
import json
import os
from hashlib import sha256
from pathlib import Path

root = Path(__file__).resolve().parent
engine = Path(os.environ['ALS_UE_SOURCE_ROOT'])
fixture = json.loads(Path('tools/tests/fixtures/vector_curve.request.json').read_text())
def request(name, value):
    (root/f'{name}.request.json').write_text(json.dumps(value,indent=2)+'\n')

for row in fixture['samples']:
    row['custom_curve_times'] = [i/20 for i in range(61)]
    row['deltas'] = [0,0.05,0.15,0.3,0.5]
request('vector', fixture)
float_rows = []
for gait in ['Walk','Run']:
    row = copy.deepcopy(fixture['samples'][0])
    row.pop('custom_curve_axis')
    row.update(name=gait, mode='Custom', custom_curve_times=[i/20 for i in range(38)],
               custom_curve=f'/ALS/ALS/Data/AnimationInstance/Grounded/CF_Als_StrideBlendAmount_{gait}')
    float_rows.append(row)
request('float', {'samples':float_rows})
request('mixed', {'samples':fixture['samples']+float_rows})
for name, axis in [('string','1'),('range',3),('fraction',0.5)]:
    bad = copy.deepcopy(fixture['samples'][1])
    bad['custom_curve_axis'] = axis
    request(name, {'samples':[bad]})
facts = {}
for file, token in [('Engine/Source/Runtime/Engine/Classes/Curves/CurveVector.h','FRichCurve FloatCurves[3]'),
                    ('Engine/Source/Runtime/Engine/Private/Curves/CurveFloat.cpp','return FloatCurve.Eval'),
                    ('Engine/Source/Runtime/Engine/Private/Curves/RichCurve.cpp','FRichCurve::Eval')]:
    raw = (engine/file).read_bytes()
    lines = raw.decode().splitlines()
    at = next(i for i,line in enumerate(lines) if token in line)
    facts[file] = {'sha256':sha256(raw).hexdigest(),'start_line':at+1,'excerpt':lines[at:at+5]}
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Prepared three-axis, old float, and three invalid native requests',flush=True)
