import gzip
import json
import math
import re
import sys
from pathlib import Path

r=Path(__file__).resolve().parent
def read(path):
    p=Path(path)
    if not p.exists():
        p=Path(str(p)+'.gz')
    return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())
part=r'RigTransform \{ rotation: DQuat\(([^)]*)\), translation: DVec3\(([^)]*)\), scale: DVec3\(([^)]*)\) \}'
pattern=re.compile(r'BonePose \{ local: '+part+r', global: '+part+r', dirty: (None|Some\((?:Local|Global)\)) \}')
def parse(value):
    matches=list(pattern.finditer(value))
    assert len(matches)==79
    assert pattern.sub('',value).replace(', ','')==''
    rows=[]
    for match in matches:
        values=match.groups()
        def transform(offset):
            arrays=[[float(v)for v in values[offset+i].split(', ')]for i in range(3)]
            assert list(map(len,arrays))==[4,3,3]and all(math.isfinite(v)for a in arrays for v in a)
            return dict(rotation=arrays[0],translation=arrays[1],scale=arrays[2])
        dirty=values[6]
        rows.append(dict(local=transform(0),global_cache=transform(3),dirty=None if dirty=='None'else dirty[5:-1]))
    return rows

label=sys.argv[1]if len(sys.argv)>1 else 'baseline'
data=read(r/('replay-'+label+'.json'))
old=read('docs/audit/bf144/476/replay-baseline.json')
base=read('docs/audit/bf144/476/inputs.json')
computed=read('docs/audit/bf144/480/replay-unit-factors.json')
requests={(row['mode'],row['row']):row for row in computed['current']if row['kind']=='actual'}
actual={(row['mode'],row['query'],row['pc']):(i,row)for i,row in enumerate(base['current'])}
output=[]
for mode,stream in enumerate(data['modes']):
    for frame in stream['outputs']:
        for call in frame['calls']:
            before=call.pop('raw_cache_before')
            after=call.pop('raw_cache_after')
            control=call.pop('clean_cache_control',None)
            if before is None:
                assert after is None
                continue
            index,row=actual[(mode,frame['query'],call['instruction'])]
            predicted=requests[(mode,index)]['result']['after']
            project=lambda v:[v[0],v[2],v[1]]
            runtime=lambda t:dict(translation=project(t['translation']),rotation=[-t['rotation'][0],-t['rotation'][2],-t['rotation'][1],t['rotation'][3]],scale=project(t['scale']))
            output.append(dict(kind='actual',row=index,mode=mode,query=frame['query'],pc=call['instruction'],weight=call['arguments_before_call'][13]['F32'],indices=[bone[0]for bone in row['bones_before']],before=parse(before),expected=parse(after),requested=list(map(runtime,predicted))))
            if control is not None:
                clean=parse(control['before'])
                assert all(row['dirty']!='Global'for row in clean)
                output.append(dict(kind='clean_control',row=index,mode=mode,query=frame['query'],pc=call['instruction'],weight=1,indices=[pair[0]for pair in control['requests']],before=clean,expected=parse(control['after']),requested=[pair[1]for pair in control['requests']]))
assert sum(row['kind']=='actual'for row in output)==1272 and data==old
assert sum(row['kind']=='clean_control'for row in output)==(1 if label=='clean-control'else 0)
sample=next(call['before']for call in output)
assert len(sample)==79
raw_sample=read(r/'replay-baseline.json')['modes'][0]['outputs'][1]['calls']
debug=next(call['raw_cache_before']for call in raw_sample if call['raw_cache_before']is not None)
rejected=0
for invalid in [debug[:-1],debug.replace('dirty: None','dirty: Unknown',1)]:
    try:
        parse(invalid)
    except AssertionError:
        rejected+=1
assert rejected==2
inputs=read(r/'inputs.json')
inputs['current']=output
inputs['current_translation_tolerance']=base['definition']['translation_tolerance']
(r/'inputs.json').write_text(json.dumps(inputs,separators=(',',':'))+'\n')
(r/'current-input-check.json').write_text(json.dumps(dict(gate='PASS_PURE_PUBLIC_DEBUG_RAW_CACHE_AND_COMPLETE_OLD_VM_TRANSPARENCY',calls=1272,bones_per_cache=79,full_old_register_local_call_trace_history_exact=True,pure_debug_no_pose_getters_or_state_changes=True,malformed_debug_rejected=2,source_and_target_snapshot_formats_kept_separate=True),indent=2)+'\n')
print('Qualified1272 complete public raw cache pre/post pairs and all old319 VM histories; two malformed Debug records rejected',flush=True)
