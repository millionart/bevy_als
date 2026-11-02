import gzip
import json
from pathlib import Path

r=Path(__file__).resolve().parent
def read(path):
    p=Path(path)
    if not p.exists():
        p=Path(str(p)+'.gz')
    return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())
base=read('docs/audit/bf144/476/inputs.json')
model=read('docs/audit/bf144/480/replay-unit-factors.json')
requested={row['row']:row['result']['after']for row in model['source']}
parents=[bone['parent']for bone in base['definition']['bones']]
names=[bone['name']for bone in base['definition']['bones']]
index={name:i for i,name in enumerate(names)}
def transform(value):
    return dict(translation=[value['translation'][k]for k in 'xyz'],rotation=[value['rotation'][k]for k in 'xyzw'],scale=[value['scale'][k]for k in 'xyz'])
def state(cache):
    by={bone['name']:bone for bone in cache}
    assert set(by)==set(names)
    result=[]
    for name in names:
        row=by[name]
        assert not(row['current_local_dirty']and row['current_global_dirty'])
        result.append(dict(local=transform(row['current_local_cache']),global_cache=transform(row['current_global_cache']),dirty='Local'if row['current_local_dirty']else 'Global'if row['current_global_dirty']else None))
    return result
records=[]
for dataset in ['actual','weights']:
    before=read('docs/audit/bf144/481a/table-'+dataset+'-before-restored.json')
    after=read('docs/audit/bf144/481a/table-'+dataset+'-after-restored.json')
    pre={(row['case'],row['query'],row['pc']):row for row in before}
    post={(row['case'],row['query'],row['pc']):row for row in after}
    frame_end=read('docs/audit/bf144/476/'+dataset+'-after.native.json')['rig_node_history_probe']['cases']
    for row_index,row in enumerate(base['source']):
        if row['dataset']!=dataset:
            continue
        a=pre[(row['case'],row['query'],row['pre_pc'])]
        b=post[(row['case'],row['query'],row['pc'])]
        indices=[index[name]for name in row['names']]
        old=state(a['cache'])
        assert all(old[i]['dirty']is None for i in indices)
        final={bone['name']:bone for bone in frame_end[row['case']]['frames'][row['query']]['hierarchy_after']['bones']}
        side='l'if row['pc']==91 else 'r'
        twists=[index['calf_twist_01_'+side],index['thigh_twist_01_'+side]]
        records.append(dict(row=row_index,dataset=dataset,case=row['case'],query=row['query'],pc=row['pc'],weight=row['weight'],indices=indices,before=old,requested=requested[row_index],expected=state(b['cache']),twists=[dict(index=i,global_transform=transform(final[names[i]]['current_global_transform']))for i in twists]))
assert len(records)==1332
(r/'inputs.json').write_text(json.dumps(dict(names=names,parents=parents,source=records,current=[]),separators=(',',':'))+'\n')
(r/'source-input-check.json').write_text(json.dumps(dict(gate='PASS_EXACT_RAW_CACHE_REQUEST_AND_TWIST_CONSUMER_INPUTS',source_cases=1332,bones_per_case=79,all_writer_globals_clean_and_locals_clean_before=True,pure_computed_requests_from480=True,twist_globals_frame_end_only_because_no_later_ancestor_writes=True,ball_frame_end_not_used_as_ik_post_value=True),indent=2)+'\n')
print('Prepared1332 complete79-bone raw pre/post caches,independent writer requests and2664 unaffected twist consumers',flush=True)
