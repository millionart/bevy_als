import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'tools/animgraph_authoring')
from ue_control_rig_contract import compile_contract
from ue_control_rig_target import compile_target
r=Path(__file__).resolve().parent

def read(path):
 p=Path(path)
 if not p.exists():p=Path(str(p)+'.gz')
 return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())

rig=read(r/'rig.native.json');contract=compile_contract(rig);prior=read('docs/audit/bf144/473/inputs.json');definition=prior['definition'];assert compile_target(contract)['program']['instructions']==definition['program']['instructions'];source=[];lookup={}
for dataset,path in[('actual','docs/audit/bf144/469a/restored.native.json.gz'),('weights','docs/audit/bf144/473/weights.native.json.gz')]:
 cases=read(path)['rig_node_history_probe']['cases']
 for case in(cases[:1]if dataset=='actual'else cases):
  assert case['adapter_enabled']and case['adapter_ready']
  for query,frame in enumerate(case['frames']):
   if query==0:continue
   hierarchy={bone['name']:bone for bone in frame['hierarchy_after']['bones']};before={bone['name']:bone['local_transform']for bone in frame['before']['bones']};after={bone['name']:bone['local_transform']for bone in frame['after']['bones']}
   for name in['foot_l','foot_r']:
    parent=hierarchy[name]['parent'];assert parent==('calf_l'if name=='foot_l'else'calf_r');row=dict(dataset=dataset,case=case['name'],query=query,foot=name,parent_name=parent,global_transform=hierarchy[name]['current_global_transform'],parent_global=hierarchy[parent]['current_global_transform'],before_local=before[name],native_local=after[name]);source.append(row)
    if dataset=='actual':lookup[(query,name)]=row
assert len(source)==696
old=read('docs/audit/bf144/473/replay-baseline.json.gz');rotation=read('docs/audit/bf144/469/inputs.json');rotations={(row['query'],row['pc']):row for row in rotation['rows']if row['dataset']=='actual'};actual=[]
for entry in read('docs/audit/bf144/473/actual.json')['rows']:
 query=entry['query'];mode=entry['mode'];foot=entry['foot'];src=lookup[(query,foot)];pc=98 if foot=='foot_l'else 128;pose=rotations[(query,pc)]['target_domains'][mode]['bones_before'];calf_index=next(i for i,bone in enumerate(definition['bones'])if bone['name']==src['parent_name']);parent=next(t for i,t in pose if i==calf_index);local=next(row['local']for row in old['modes'][mode]['outputs'][query]['local']if row['name']==foot);actual.append(dict(mode=mode,query=query,foot=foot,parent_name=src['parent_name'],global_transform=entry['observed'],parent_global=parent,expected_local=local,source=src))
assert len(actual)==1272
flags={}
for kind,path in[('actual','docs/audit/bf144/474a/table-restored.json'),('weights','docs/audit/bf144/474a/table-weights-restored.json')]:
 case_names=[c['name']for c in read('docs/audit/bf144/474a/weights.request.json')['cases']]if kind=='weights'else[]
 for row in read(path):
  if kind=='actual'and row['case']!=0:continue
  name=case_names[row['case']]if kind=='weights'else'actual319-reference-0';flags[(kind,name,row['query'],row['foot'])]=row['local_dirty_before_global_read']
for row in source:row['local_dirty']=flags[(row['dataset'],row['case'],row['query'],row['foot'])]
def ue_current(t):
 x,y,z,w=t['rotation'];a,b,c=t['translation'];i,j,k=t['scale'];return dict(rotation=dict(x=-x,y=-z,z=-y,w=w),translation=dict(x=a*100,y=c*100,z=b*100),scale=dict(x=i,y=k,z=j),translation_runtime=t['translation'])
for row in actual:row['current_global_source_axes']=ue_current(row['global_transform']);row['current_parent_source_axes']=ue_current(row['parent_global'])
(r/'inputs.json').write_text(json.dumps(dict(definition=definition,source=source,actual=actual),separators=(',',':'))+'\n');print('Prepared696 exact source post-global/parent/final-local and1272 actual target local boundaries',flush=True)
