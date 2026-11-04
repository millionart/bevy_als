import gzip
import hashlib
import json
import os
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/500')
temporary.mkdir(parents=True,exist_ok=True)
with gzip.open(parent/'499/replay-restored.json.gz','rt') as stream:
    previous = json.load(stream)
assert previous['minimum_source_field_mismatches']==0 and len(previous['best_source_models'])==96
modes = [previous['rows'][0]['native'][index]['mode'] for index in previous['best_source_models']]
(temporary/'modes.json').write_text(json.dumps(modes,separators=(',',':')))
prior = (parent/'499/replay.rs').read_text()
a = prior.index('fn a32')
b = prior.index('fn main()',a)
math = prior[a:b]
start = math.index('fn target_direction(')
end = math.index('fn native_lerp(',start)
(temporary/'math.rs').write_text(math[:start]+math[end:])
print('Qualified real VelocityBlend endpoints and all 96 native-calibrated variants',flush=True)

engine = Path(os.environ['ALS_UE_SOURCE_ROOT'])
config = json.loads(Path('tools/env.local.json').read_text())
als = Path(config['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
facts = []
for base,key,ranges in [
    (als,'Private/AlsAnimationInstance.cpp',[(694,705),(745,777),(824,827),(850,901)]),
    (engine,'Engine/Source/Runtime/Core/Public/Math/Quat.h',[(1252,1258)]),
    (Path('.'),'tools/import/coordinate_basis.py',[(5,47)]),
    (Path('.'),'src/als/utility/orientation.rs',[(3,13)]),
    (Path('.'),'src/als/character/animation_instances/ab_als_provider.rs',[(250,256),(313,323)]),
    (Path('.'),'src/als/character/animation_instances/source_native_refresh.rs',[(135,183)]),
]:
    raw = (base/key).read_bytes()
    lines = raw.decode('utf-8-sig').splitlines()
    facts.append(dict(key=f'source:{key}' if base==als else key,sha256=hashlib.sha256(raw).hexdigest(),
                      excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b])) for a,b in ranges]))
(root/'source-facts.json').write_text(json.dumps(facts,indent=2)+'\n')
print('Captured native world velocity and rotation sources and current runtime basis contract',flush=True)
