import gzip
import hashlib
import json
from collections import Counter
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/493')
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.resolve().relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

source = read('source-check.json')
assert source['native_frames']==158 and source['exact_sockets']==316
assert source['exact_final_getter_rig_fields']==632 and source['wrong_marker_rejections']==316
assert source['local_pose_changes']=={
    'rig_source_to_rig_output/VB foot_root':52,'rig_output_to_root_result/VB foot_root':0,
    'rig_source_to_rig_output/VB foot_l':0,'rig_output_to_root_result/VB foot_l':0,
    'rig_source_to_rig_output/VB foot_r':0,'rig_output_to_root_result/VB foot_r':0}
def case(label):
    return read(f'{label}.native.json')['character_animation_probe']['cases'][0]
short,long = case('virtual'),case('virtual-motion')
assert short['initialization']==long['initialization'] and short['frames']==long['frames'][:80]
assert all(short[key]==long[key] for key in short if key not in ['frames','initialization'])
del short,long
for label in ['virtual','virtual-motion']:
    assert read(f'{label}.native.json.reader.report.json')['success']

target = {}
for label in ['virtual','default']:
    target[label] = read(f'target-{label}-check.json')
    assert target[label]['observed_frames']==159 and target[label]['valid_frames']==158
    assert target[label]['exact_chain_nodes']==1264 and target[label]['rig_binding_fields']==632
    assert read(f'runtime-{label}/process.report.json')['success']
    gate = read(f'{label}-functional.json')
    assert gate['pass'] and not gate['issues'] and not gate['images'] and gate['validation_scope']=='functional'
    replay = read(f'replay-unit-target-{label}.json')
    assert not replay['failures'] and len(replay['rows'])==158
    assert read(f'replay-target-{label}.report.json')['success']
assert target['virtual']['local_pose_changes']['rig_source_to_rig_output']=={'VB foot_root':52,'VB foot_l':0,'VB foot_r':0}
assert Counter(row['bone'] for row in read('root-blend-virtual.json'))=={'VB foot_l':3,'VB foot_r':9}
assert Counter(row['bone'] for row in read('root-blend-default.json'))=={'ik_foot_l':4,'ik_foot_r':1}
assert not read('replay-root-blend-passthrough.report.json')['success']
assert read('replay-compile-root-blend-passthrough.report.json')['success']
for label in ['virtual','default']:
    assert read(f'replay-root-blend-{label}.report.json')['success']
assert (root/'replay-unit-target-virtual.json').read_bytes()==(root/'replay-unit-target-virtual-restored.json').read_bytes()
assert read('replay-target-virtual-restored.report.json')['success']
negative = read('replay-unit-target-virtual-ik.json')
assert Counter(row['field'] for row in negative['failures'])=={'states':158,'targets':158,'rig':61}
assert not read('replay-target-virtual-ik.report.json')['success']
assert len({row['query'] for row in negative['failures']})==158
print('Verified native and actual target producers, control failures and two functional gates',flush=True)

decoded = {}
files = ['virtual-motion.native.json',
         *[f'replay-unit-target-{label}.json' for label in ['default','virtual','virtual-ik','virtual-restored']],
         *[f'runtime-{label}/{name}.json' for label in ['virtual','default'] for name in ['parameters','report']]]
for name in files:
    path = root/name
    print(f'Verifying compressed original {name}',flush=True)
    raw = path.read_bytes()
    packed = gzip.compress(raw,mtime=0)
    assert gzip.decompress(packed)==raw
    path.with_suffix('.json.gz').write_bytes(packed)
    decoded[name] = dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

references = ['457/request.json','459/inputs.json','484/playground_observed.rs','484/foot_observer.rs','484/replay.rs',
              '484/runtime-observed/parameters.json.gz','486/replay.rs','485/native.rs','479/replay.rs','478/replay.rs',
              '471/replay.rs','491/reader-migration.json','441c/library-identities.json']
summary = dict(gate='PASS_ACTUAL_VIRTUAL_FOOT_POSE_PUBLICATION_AND_CONSUMERS',
               new_differences=0,known_total=276,native=source,target=target,
               virtual_root_blend_rotations_existing_D245=12,
               default_scene_baseline_prefix_exact=True,virtual_wrong_selector_frames=158,
               source_original_80_prefix_exact=True,whole_cross_engine_pose_equivalence_claimed=False,
               screenshots=0,performance_acceptance='paused',decoded=decoded,
               generated_sources=[identity(temporary/name) for name in ['playground_observed.rs','foot_observer.rs','target.rs']],
               external_source_files=[identity(parent/path) for path in references])
(root/'final-source.json').write_text(json.dumps(identity(root/'replay.rs'),indent=2)+'\n')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified replay and original evidence hashes',flush=True)
