import gzip
import hashlib
import json
import re
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/491')
reader = Path('tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp')
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

def native(label):
    return read(f'{label}.native.json')['character_animation_probe']

labels = ['switch-baseline','switch-restored','no-initial-write','no-frame-write','legacy-before','legacy-after']
for label in labels+['invalid-rejected']:
    report = read(f'{label}.native.json.reader.report.json')
    assert report['process_exit_code']==(7 if label=='invalid-rejected' else 0)
    assert report['success']==(label!='invalid-rejected')
assert not (root/'invalid-rejected.native.json').exists()
assert native('legacy-before')==native('legacy-after')
assert native('switch-baseline')==native('switch-restored')
for name in ['lifecycle','socket']:
    assert (root/f'{name}-switch-baseline.json').read_bytes()==(root/f'{name}-switch-restored.json').read_bytes()
assert not read('socket_prepare-mixed-case-correction.report.json')['success']
old = {case['name']:case for case in native('legacy-before')['cases']}
def normalize_world(value):
    return re.sub(r'/Temp/Untitled_\d+\.Untitled:', '/Temp/Untitled.Untitled:', json.dumps(value,sort_keys=True))
control,initial_ik,_ = native('switch-restored')['cases']
assert normalize_world(control)==normalize_world(old['far-shifted-floor-and-quarter-turn'])
legacy_ik = old['actual-positive-history318-with-initial-pose']
def without_settings(value, state_key):
    return {**value,state_key:[field for field in value[state_key] if field['name']!='Settings']}
assert normalize_world(without_settings(initial_ik['initialization'],'source_state')) == normalize_world(
    without_settings(legacy_ik['initialization'],'source_state'))
for actual,legacy in zip(initial_ik['frames'][:2],legacy_ik['frames'][:2],strict=True):
    assert normalize_world(without_settings(actual,'source_state_after_tick')) == normalize_world(
        without_settings(legacy,'source_state_after_tick'))
print('Verified old request and restored complete native probe equality',flush=True)

def lifecycle(label):
    rows = read(f'lifecycle-{label}.json')
    assert len(rows)==2
    return rows

restored = lifecycle('switch-restored')
for case,initial in zip(restored,[True,False],strict=True):
    bone = 'ik_foot_root' if initial else 'VB foot_root'
    assert case['initial']==dict(getter_flag=initial,input_flag=initial,foot_root=bone)
    assert case['settings']!=old['actual-positive-history318-with-initial-pose']['settings']
    assert len(case['frames'])==8
    for frame in case['frames']:
        assert frame['requested']==frame['getter_before']==frame['getter_after']==frame['input_flag']
        assert frame['foot_root']==bone
        assert frame['executions']==[{'entry':'Forwards Solve','succeeded':True}]

initial_negative = lifecycle('no-initial-write')
assert initial_negative[0]==restored[0]
assert initial_negative[1]['initial']==dict(getter_flag=True,input_flag=True,foot_root='ik_foot_root')
assert [row['frame'] for row in initial_negative[1]['frames'] if row['input_flag']!=row['requested']]==[0,1]
assert all(row['foot_root']=='ik_foot_root' for row in initial_negative[1]['frames'])
frame_negative = lifecycle('no-frame-write')
for negative,positive in zip(frame_negative,restored,strict=True):
    assert negative['initial']==positive['initial']
    assert [row['frame'] for row in negative['frames'] if row['input_flag']!=row['requested']]==[2,3,4]
    assert all(row['foot_root']==positive['initial']['foot_root'] for row in negative['frames'])
print('Verified initial and runtime-write ablations fail their respective lifecycle contracts',flush=True)

socket = read('socket-switch-restored.json')
assert len(socket)==32 and sum(row['valid'] for row in socket)==28
for row in socket:
    name = ('ik_foot_' if row['requested'] else 'VB foot_')+['l','r'][row['side']]
    phase = 'prior' if row['valid'] else 'current'
    selected = next(c for c in row['controls'] if c['bone']==name and c['phase']==phase)
    wrong = next(c for c in row['controls'] if c['bone']!=name and c['phase']==phase)
    assert selected['t_exact'] and selected['q_exact']
    assert not (wrong['t_exact'] and wrong['q_exact'])
print('Verified all 32 source socket choices and the rejected alternate marker',flush=True)

before = (temporary/'reader.before.cpp').read_bytes()
canonical = (temporary/'reader.final.cpp').read_bytes()
assert reader.read_bytes()==canonical
prior_manifest = 'docs/audit/bf144/481a/reader-migration.json'
prior = json.loads(Path(prior_manifest).read_text())
before_sha = hashlib.sha256(before).hexdigest()
after_sha = hashlib.sha256(canonical).hexdigest()
assert before_sha==prior[reader.as_posix()]['after_sha256']!=after_sha
for label in ['baseline','restored','no-initial-write','no-frame-write']:
    package = read(f'package-{label}.json')
    assert package['source_sha256']==package['package_source_sha256']
    assert (package['source_sha256']==after_sha)==(label in ['baseline','restored'])
    assert read(f'build-{label}.report.json')['success']
assert read('package-no-initial-write.json')['source_sha256']!=read('package-no-frame-write.json')['source_sha256']
migration = {reader.as_posix():dict(before_sha256=before_sha,after_sha256=after_sha,
                                 baseline_migration=prior_manifest,
                                 scope='Opt-in transient native per-instance foot setting for character probe only')}
(root/'reader-migration.json').write_text(json.dumps(migration,indent=2)+'\n')

decoded = {}
for label in labels:
    path = root/f'{label}.native.json'
    raw = path.read_bytes()
    packed = gzip.compress(raw,mtime=0)
    assert gzip.decompress(packed)==raw
    path.with_suffix('.json.gz').write_bytes(packed)
    decoded[path.name]=dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
    print(f'Verified lossless {label} bytes',flush=True)
references = ['457/request.json','486/replay.rs','485/native.rs','479/replay.rs','478/replay.rs','471/replay.rs',
              '481a/reader-migration.json','441c/library-identities.json']
summary = dict(gate='PASS_NATIVE_FOOT_SETTING_SWITCH_AND_CONSTRUCTION_LATCH',new_differences=0,
               native_cases=2,native_frames=16,live_switches=4,exact_socket_samples=32,
               exact_prior_valid_samples=28,exact_current_initial_samples=4,
               wrong_marker_rejections=32,constructs_after_begin_play=0,
               source_legacy_probe_exact=True,target_existing_runtime_compared=False,
               screenshots=0,performance_acceptance='paused',decoded=decoded,
               external_source_files=[identity(parent/path) for path in references])
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
