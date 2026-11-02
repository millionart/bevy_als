import copy
import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'docs/audit/bf144/462')
from cache_identity import cache_identity

r=Path(__file__).resolve().parent
label=sys.argv[1]
def read(path):
    p=Path(path)
    if not p.exists():
        p=Path(str(p)+'.gz')
    return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())
kind=next(kind for kind in ['actual-before','actual-after','weights-before','weights-after']if label.startswith(kind))
new=read(r/(label+'.native.json'))['rig_node_history_probe']
old=read(Path('docs/audit/bf144/476')/(kind+'.native.json'))['rig_node_history_probe']
base=read('docs/audit/bf144/476/inputs.json')
parents={bone['name']:(base['definition']['bones'][bone['parent']]['name']if bone['parent']is not None else 'None')for bone in base['definition']['bones']}
source={(row['dataset'],row['case'],row['query'],row['pc']):row for row in base['source']}
records=[]
counts=dict(snapshots=0,bones=0,global_dirty=0,local_dirty=0,clean_typed_global_exact=0,active_leg_ball_dirty=0)
for ci,(case,prior)in enumerate(zip(new['cases'],old['cases'],strict=True)):
    assert case['unit_boundary_binding_restored']
    assert {k:v for k,v in case.items()if k!='frames'}=={k:v for k,v in prior.items()if k!='frames'}
    for qi,(frame,before)in enumerate(zip(case['frames'],prior['frames'],strict=True)):
        if qi%64==0:
            print('Checking raw hierarchy cache '+kind+' case='+str(ci)+' frame='+str(qi),flush=True)
        clean=copy.deepcopy(frame)
        for boundary in clean['unit_boundaries']:
            snapshot=boundary.pop('hierarchy_cache_before_item_reads')
            by={row['name']:row for row in snapshot}
            assert len(snapshot)==len(by)==79 and set(by)==set(parents)
            for row in snapshot:
                assert row['type']=='Bone'and row['parent']==parents[row['name']]
                assert type(row['current_local_dirty'])is type(row['current_global_dirty'])is bool
                assert not(row['current_local_dirty']and row['current_global_dirty'])
                assert set(row['current_local_cache'])==set(row['current_global_cache'])=={'translation','rotation','scale'}
                counts['bones']+=1
                counts['global_dirty']+=row['current_global_dirty']
                counts['local_dirty']+=row['current_local_dirty']
            for item in boundary['items']:
                if not item['valid_transform_item']:
                    continue
                cache=by[item['name']]
                assert cache['current_local_dirty']==item['current_local_dirty_before_global_read']
                if not cache['current_global_dirty']:
                    assert cache['current_global_cache']==item['current_global_transform']
                    counts['clean_typed_global_exact']+=1
            pc=boundary['instruction']
            if kind.endswith('after')and pc in [91,121]:
                row=source[('weights'if kind.startswith('weights')else 'actual',ci,qi,pc)]
                if row['weight']>1e-8:
                    ball=by['ball_l'if pc==91 else 'ball_r']
                    assert ball['current_global_dirty']and not ball['current_local_dirty'],'Active IK child propagation must leave the untouched ball global dirty and local clean'
                    counts['active_leg_ball_dirty']+=1
            records.append(dict(case=ci,query=qi,pc=pc,function=boundary['function'],cache=snapshot))
            counts['snapshots']+=1
        clean['work_after']=cache_identity(clean['work_after'])
        expected=copy.deepcopy(before)
        expected['work_after']=cache_identity(expected['work_after'])
        assert clean==expected,(kind,ci,qi)
expected_snapshots={'actual-before':1272,'weights-before':60,'actual-after':2548,'weights-after':130}[kind]
assert counts['snapshots']==expected_snapshots
assert counts['bones']==expected_snapshots*79
(r/('check-'+label+'.json')).write_text(json.dumps(dict(gate='PASS_READONLY_RAW_HIERARCHY_CACHE_AND_COMPLETE_OLD_HISTORY_TRANSPARENCY',kind=kind,counts=counts,all_old_fields_and_execution_pose_work_exact=True,raw_dirty_cache_not_claimed_evaluated=True),indent=2)+'\n')
(r/('table-'+label+'.json')).write_text(json.dumps(records,separators=(',',':'))+'\n')
print(json.dumps(counts),flush=True)
