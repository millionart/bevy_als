import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target
r=Path(__file__).resolve().parent;rig=json.loads(gzip.decompress((r/'rig.native.json.gz').read_bytes()));base=json.loads(Path('docs/audit/bf144/469/inputs.json').read_bytes());assert compile_target(compile_contract(rig))['program']['instructions']==base['definition']['program']['instructions'];normal=json.loads(Path('docs/audit/bf144/469/replay-restored.json').read_bytes())['normals'];relative=json.loads(Path('docs/audit/bf144/470/replay-restored.json').read_bytes())['outputs'];rows=[];q=lambda obj:[obj[k]for k in'xyzw']
for source,native_normal,native_relative in zip(base['rows'],normal,relative,strict=True):
 key=[source['dataset'],source['query'],source['pc']];assert key==[native_normal[k]for k in['dataset','query','pc']]==[native_relative[k]for k in['dataset','query','pc']];args=source['source_arguments_after'];assert native_normal['after']==args[11]and native_relative['native']==args[13]
 rows.append(dict(delta_seconds=source['delta_seconds'],calf_name=args[0][1],foot_name=args[1][1],raw_normal=args[3],half_life=args[7],calf_global=source['source_calf_global'],foot_global=source['source_foot_global_at_call'],calf_initial_global=source['source_calf_initial_global'],foot_initial_global=source['source_foot_initial_global'],dataset=key[0],query=key[1],pc=key[2],calf=q(source['source_calf_global']['rotation']),foot=q(source['source_foot_global_at_call']['rotation']),target=args[2],normal=native_normal['after'],initial_relative=native_relative['native'],limits=[args[5],args[4],args[6]],expected_native=args[12]))
assert len(rows)==684
(r/'inputs.json').write_text(json.dumps(dict(definition=base['definition'],rows=rows),separators=(',',':'))+'\n');print('Prepared684 complete source rotations using qualified bone inputs and independently predicted normal/initial relative stages',flush=True)
