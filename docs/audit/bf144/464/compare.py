import collections
import gzip
import json
import struct
import sys
from pathlib import Path

r=Path(__file__).resolve().parent;native=json.loads(gzip.decompress(Path('docs/audit/bf144/462/restored.native.json.gz').read_bytes()))['rig_node_history_probe']['cases'][0]['frames'];label=sys.argv[1]if len(sys.argv)>1 else'baseline';path=r/('replay-'+label+'.json');report=json.loads(path.read_bytes()if path.exists()else gzip.decompress(path.with_suffix('.json.gz').read_bytes()));modes=report['modes'];assert [m['source']for m in modes]==[False,True]
f32=lambda v:struct.unpack('f',struct.pack('f',v))[0]
consumers=collections.defaultdict(lambda:collections.Counter());state=collections.Counter();changed_bones=collections.Counter();first={};source_counts=[collections.Counter(),collections.Counter()];partition={k:collections.Counter()for k in ['translation','rotation','scale']};frames=[]
for query,(a,b,n)in enumerate(zip(modes[0]['outputs'],modes[1]['outputs'],native,strict=True)):
 assert a['query']==b['query']==query and a['steps']==b['steps'];state['changed_register_rows']+=sum(x!=y for x,y in zip(a['registers'],b['registers'],strict=True))
 for x,y in zip(a['calls'],b['calls'],strict=True):
  assert x['instruction']==y['instruction']and x['function']==y['function'];name=x['function'];c=consumers[name];c['calls']+=1;c['changed_before_arguments']+=x['arguments_before_call']!=y['arguments_before_call'];c['changed_after_arguments']+=x['arguments_after_call']!=y['arguments_after_call'];c['changed_pre_call_bones']+=x['bones_before_call']!=y['bones_before_call']
  if x!=y and name not in first:first[name]=dict(query=query,instruction=x['instruction'],before=x['arguments_before_call'],controlled_before=y['arguments_before_call'])
 expected={bone['name']:bone['local_transform']for bone in n['after']['bones']};local=collections.Counter()
 for x,y in zip(a['local'],b['local'],strict=True):
  assert x['name']==y['name'];e=expected[x['name']];p=e['translation'];q=e['rotation'];s=e['scale'];projected=dict(translation=[f32(p['x']*.01),f32(p['z']*.01),f32(p['y']*.01)],rotation=[f32(-q['x']),f32(-q['z']),f32(-q['y']),f32(q['w'])],scale=[f32(s['x']),f32(s['z']),f32(s['y'])])
  for key in projected:
   before=[f32(v)for v in x['local'][key]];after=[f32(v)for v in y['local'][key]];old=before==projected[key];new=after==projected[key];source_counts[0][key]+=not old;source_counts[1][key]+=not new;local['changed_'+key]+=before!=after;changed_bones[key]+=before!=after
   tag='preserved'if old and new else'regressed'if old else'recovered'if new else'still_different';partition[key][tag]+=1
 frames.append(dict(query=query,**local))
assert source_counts[0]==dict(translation=1324,rotation=1845,scale=0)
result=dict(gate='CONTINUOUS319_TRACE_VECTOR_CONTROL_CONSUMER_AND_NATIVE_RIG_OUTPUT_INVENTORY',queries=319,register_rows=129195,local_bone_rows=25201,changed_register_rows=state['changed_register_rows'],changed_projected_bones=dict(changed_bones),source_residual_original=dict(source_counts[0]),source_residual_controlled=dict(source_counts[1]),source_partition={k:dict(v)for k,v in partition.items()},consumers={k:dict(v)for k,v in consumers.items()},first_consumer_changes=first,source_full_rig_equivalence_claimed=False,new_differences=[])
(r/'comparison.json').write_text(json.dumps(result,indent=2)+'\n');(r/'frame-table.json').write_text(json.dumps(frames,separators=(',',':'))+'\n');print(json.dumps({k:v for k,v in result.items()if k not in ['first_consumer_changes','consumers']}),flush=True);print(json.dumps({k:v for k,v in result['consumers'].items()if any(v[z]for z in ['changed_before_arguments','changed_after_arguments','changed_pre_call_bones'])}),flush=True)
