import hashlib
import json
import math
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/490')
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

baseline = read('replay-restored.json')
assert (root/'replay-baseline.json').read_bytes()==(root/'replay-restored.json').read_bytes()
assert baseline['selection_mismatches']==0
assert len(baseline['records'])==15
assert [case['foot_root']['List'][1]['Name'] for case in baseline['construction']]==['ik_foot_root','VB foot_root']
forced = read('replay-force-ik.json')
assert forced['selection_mismatches']==6
for label in ['baseline','restored','force-ik','keep-marker','missing-fixture-transition-state','fixture-readiness','diagnostic']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success']==(label in ['baseline','restored'])
for label in ['force-ik','keep-marker']:
    log = (temporary/f'replay-{label}.stderr.log').read_text()
    reason = next(line for line in log.splitlines() if line.startswith('assertion '))
    detail = [line.strip() for line in log.splitlines() if line.lstrip().startswith(('left:','right:'))]
    (root/f'{label}.failure.json').write_text(json.dumps(dict(assertion=reason,details=detail),indent=2)+'\n')
assert 'Source selector and published flag must agree' in read('force-ik.failure.json')['assertion']
fallbacks = []
for query in [1,105,203]:
    steps = [row for row in baseline['records'] if row['query']==query]
    assert [row['requested'] for row in steps]==[None,False,True,None,False]
    assert [row['published'] for row in steps]==[True,False,True,True,False]
    assert all(row['coherent'] for row in steps)
    assert steps[0]['sides']==steps[2]['sides']==steps[3]['sides']
    assert [side['name'] for side in steps[1]['sides']]==['VB foot_l','VB foot_r']
    assert [side['name'] for side in steps[4]['sides']]==['foot_l','VB foot_r']
    assert steps[1]['sides'][1]==steps[4]['sides'][1]
    missing = steps[4]
    point = missing['source_missing_world_t']
    dist = math.dist(point,missing['sides'][0]['t'])
    assert 0.22<dist<0.48
    fallbacks.append(dict(query=query,source_contract_world_t=point,
                          source_contract_world_q=missing['source_missing_world_q'],
                          target_actual=missing['sides'][0],position_distance_m=dist))
references = ['484/inputs.json.gz','485/native-input.json.gz','459/inputs.json','481a/reader-migration.json',
              '441c/library-identities.json']
summary = dict(gate='PASS_FOOT_SELECTOR_SWITCH_AND_MISSING_VIRTUAL_MARKER',new_differences=1,
               difference='D279',priority='P3',known_total=276,loaded_pose_fixtures=3,
               intact_selector_frames=12,missing_marker_frames=3,selector_publication_mismatches=0,
               forced_ik_mismatches=6,missing_marker_fallbacks=fallbacks,
               native_false_role_run=False,source_missing_contract='local UE implementation',
               target_vm_construction=baseline['construction'],
               screenshots=0,performance_acceptance='paused',
               external_source_files=[identity(parent/path) for path in references])
(root/'final-source.json').write_text(json.dumps(identity(root/'replay.rs'),indent=2)+'\n')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified registered selector, both VM constructions and bounded fallback witness',flush=True)
