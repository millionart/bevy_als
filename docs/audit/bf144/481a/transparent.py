import gzip
import json
import sys
from pathlib import Path

sys.path.insert(0,'docs/audit/bf144/462')
from cache_identity import cache_identity
r=Path(__file__).resolve().parent
label=sys.argv[1]
kind=next(kind for kind in ['actual-before','actual-after','weights-before','weights-after']if label.startswith(kind))
def read(path):
    p=Path(path)
    if not p.exists():
        p=Path(str(p)+'.gz')
    return json.loads(gzip.decompress(p.read_bytes())if p.suffix=='.gz'else p.read_bytes())['rig_node_history_probe']
new=read(r/(label+'.native.json'))
old=read(Path('docs/audit/bf144/476')/(kind+'.native.json'))
snapshots=0
for body in [new,old]:
    for case in body['cases']:
        assert case['unit_boundary_binding_restored']
        for frame in case['frames']:
            frame['work_after']=cache_identity(frame['work_after'])
            for boundary in frame['unit_boundaries']:
                if body is new:
                    boundary.pop('hierarchy_cache_before_item_reads')
                    snapshots+=1
assert new==old
(r/('transparent-'+label+'.json')).write_text(json.dumps(dict(gate='PASS_COMPLETE_OLD_BODY_AFTER_REMOVING_ONLY_ADDITIVE_CACHE_SNAPSHOT',kind=kind,snapshots=snapshots,original_execution_pose_work_hierarchy_and_items_exact=True),indent=2)+'\n')
print('Verified complete old source body after only additive cache-array removal '+str(snapshots),flush=True)
