import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target
r=Path(__file__).resolve().parent;p=r/'rig.native.json';rig=json.loads(p.read_bytes()if p.exists()else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes()));base=json.loads(Path('docs/audit/bf144/474/inputs.json').read_bytes());assert compile_target(compile_contract(rig))['program']['instructions']==base['definition']['program']['instructions']
prior=json.loads(gzip.decompress(Path('docs/audit/bf144/474/replay-restored.json.gz').read_bytes()));common={(x['case'],x['query'],x['foot'],x['mask']):x for x in prior['outputs']};matrix={(x['mode'],x['query'],x['foot'],x['data_mask'],x['math_mask']):x for x in prior['matrix']}
for row in base['source']:row['qualified_models']={str(mask):common[(row['case'],row['query'],row['foot'],mask)]for mask in[0,7,8,15]}
for row in base['actual']:row['qualified_models']={str(data)+':'+str(mask):matrix[(row['mode'],row['query'],row['foot'],data,mask)]for data in[0,3]for mask in[0,7,8,15]}
(r/'inputs.json').write_text(json.dumps(base,separators=(',',':'))+'\n');print('Prepared696 common native source and1272 actual current double unit phase controls with prior source/public qualification',flush=True)
