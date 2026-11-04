import gzip
import hashlib
import json
import os
from pathlib import Path

root = Path(__file__).resolve().parent
def raw(name):
    path = root / name
    return path.read_bytes() if path.exists() else gzip.decompress(path.with_suffix('.json.gz').read_bytes())

def load(name):
    return json.loads(raw(name))

def save(name, value):
    (root / name).write_text(json.dumps(value, indent=2) + '\n')

def properties(rows):
    return {row['name']: row for row in rows}

guid = 'ccaab6a2-424d-b00d-6794-f3a705d6f8c2'
graph = next(graph for graph in load('ab_als.native.json')['graphs'] if graph['name'] == 'AnimGraph')
node = next(node for node in graph['nodes'] if node['node_guid'] == guid)
slot_props = properties(properties(node['properties'])['Node']['fields'])
assert slot_props['SlotName']['exported_text'] == 'PostLocomotion'
assert slot_props['bAlwaysUpdateSourcePose']['exported_text'] == 'False'
target = Path('assets/ALS/Character/AB_Als/AnimGraph.animgraph.ron')
target_node = next(line for line in target.read_text().splitlines() if guid in line and 'SourceSlotNode' in line)
assert 'slot_name: "PostLocomotion"' in target_node and 'always_update_source_pose: true' not in target_node
for name in ('ab_als', 'landing'):
    assert load(f'{name}.native.json.reader.report.json')['success']

def source_rows(frames):
    result = []
    for row in frames:
        main = next(item for item in row['linked_instances'] if item['main'])
        ground = next(item for item in row['linked_instances'] if item['class'].endswith('.AB_Als_Grounded_C'))
        slot = next(item for item in main['slots'] if item['slot_name'] == 'PostLocomotion')
        b = properties(properties(properties(row['source_state_after_tick'])['GroundedState']['fields'])['VelocityBlend']['fields'])
        montage = [item for item in main['montage_evaluation'] if any(
            '/Roll/' in segment['sequence'] for track in item['tracks'] for segment in track['segments'])]
        assert len(montage) <= 1
        result.append(dict(frame=row['frame'], mode=row['movement_after_tick']['movement_mode_name'],
                           update=ground['update_counter'], source_weight=slot['source_weight'],
                           slot_weight=slot['slot_node_weight'],
                           always_update=slot['always_update_source_pose'],
                           velocity=row['actor_velocity_cm_s'], delta=row['world_delta_seconds'],
                           amounts=[b[direction + 'Amount']['numeric_value'] for direction in ('Forward', 'Backward', 'Left', 'Right')],
                           roll=montage[0] if montage else None))
    return result

native = source_rows(load('landing.native.json')['character_animation_probe']['cases'][0]['frames'])
old = json.loads(gzip.decompress((root.parent / '505/landing.native.json.gz').read_bytes()))
assert source_rows(old['character_animation_probe']['cases'][0]['frames']) == native[:52]
assert len(native) == 72 and all(not row['always_update'] for row in native)
assert native[24]['mode'] == 'MOVE_Falling' and native[25]['mode'] == 'MOVE_Walking'
assert -1000 < native[24]['velocity']['z'] <= -700
assert [i for i, row in enumerate(native) if row['source_weight'] == 0] == list(range(27, 51))
assert all(row['update'] == native[26]['update'] and row['amounts'] == native[26]['amounts'] for row in native[27:51])
assert native[51]['update'] > native[50]['update'] and native[51]['amounts'] != native[50]['amounts']
assert native[25]['roll'] and native[58]['roll'] and not native[59]['roll']
save('native-slot.json', native)
print('Verified native slot binding and 72-frame landing trace', flush=True)

assert not load('compile-too-many-parameters.report.json')['success']
assert load('runtime-uncompiled/process.report.json')['exit_reason'] == 'start_failed'
assert load('runtime-rejected-full-blend/process.report.json')['process_exit_code'] == 101
assert load('rejected-control.json')['non_finite_quaternion']
runs = {}
for label in ('baseline', 'masked-source', 'restored'):
    for report in (f'{label}-functional-process.json', f'compile-{label}.report.json', f'runtime-{label}/process.report.json'):
        assert load(report)['success'], report
    check = load(f'{label}-functional.json')
    assert check['pass'] and check['validation_scope'] == 'functional' and check['images'] == []
    data = load(f'runtime-{label}/parameters.json')
    assert len(data['before']) == len(data['after']) == 96
    runs[label] = {side: {row['ordinal']: row for row in data[side]} for side in ('before', 'after')}
assert raw('runtime-baseline/parameters.json') == raw('runtime-restored/parameters.json')
base = runs['baseline']
ablation = runs['masked-source']
assert -10 < base['after'][58]['velocity'][1] <= -7
assert base['after'][58]['gameplay'] == 'Air' and base['after'][59]['gameplay'] == 'Grounded'
assert not base['after'][58]['roll']['active'] and base['after'][59]['roll']['active']
assert base['after'][59]['action'].endswith('.Rolling')
assert [i for i, row in base['after'].items() if row['post_locomotion']['weight'] == 1] == list(range(61, 85))
for i in range(61, 85):
    base_row = base['after'][i]
    altered = ablation['after'][i]
    a, b = base_row['velocity_blend'], altered['velocity_blend']
    assert base_row['post_locomotion'] == altered['post_locomotion']
    assert base_row['roll'] == altered['roll']
    assert base_row['position'] == altered['position'] and base_row['velocity'] == altered['velocity']
    assert not a['calls'] and a['write_frame'] == 140
    assert a['amounts'] == base['after'][60]['velocity_blend']['amounts']
    assert len(b['calls']) == 2 and b['write_frame'] == altered['animation_frame']
    assert a['instance_id'] == b['instance_id']
    assert base_row['graph_error'] is None and altered['graph_error'] is None
assert all(base['after'][i] == ablation['after'][i] for i in range(24, 61))
assert ablation['after'][84]['velocity_blend']['amounts'] != base['after'][84]['velocity_blend']['amounts']
return_values = base['after'][85]['velocity_blend']['amounts']
candidates = base['before'][85]['velocity_blend']['candidates']
snap_error = max(abs(a - b) for a, b in zip(return_values, candidates['snap'], strict=True))
damp_error = max(abs(a - b) for a, b in zip(return_values, candidates['damp'], strict=True))
assert snap_error <= 1e-8 and damp_error > 0.25
assert base['after'][85]['velocity_blend']['write_frame'] == 165
assert base['after'][86]['velocity_blend']['amounts'] == base['before'][86]['velocity_blend']['candidates']['damp']
assert base['after'][92]['roll']['active'] and not base['after'][93]['roll']['active']

comparison = []
for offset in range(35):
    s, t = native[25+offset], base['after'][59+offset]
    assert t['velocity_blend']['delta'] == s['delta']
    source_updated = s['update'] != native[24+offset]['update']
    assert source_updated == bool(t['velocity_blend']['calls'])
    weight = t['post_locomotion']['weight']
    comparison.append(dict(offset=offset, native=s['slot_weight'], target=weight,
                           difference=weight - s['slot_weight'], grounded_updated=source_updated))
changed = [row['offset'] for row in comparison if row['difference']]
assert changed == list(range(26, 34))
save('slot-comparison.json', comparison)
source = root / 'slot_observer.rs'
backup = Path('tools/tmp/bf144/506/slot_observer.final.rs')
if backup.exists():
    assert source.read_bytes() == backup.read_bytes()
save('final-source.json', dict(sha256=hashlib.sha256(source.read_bytes()).hexdigest()))
print('Verified three visible controls and aligned Grounded update masks', flush=True)

facts = []
project = json.loads(Path('tools/env.local.json').read_text())
als = Path(project['ue_project']).parent/'Plugins/ALS-Refactored/Source/ALS'
ue = Path(os.environ['ALS_UE_SOURCE_ROOT'])
for key, path, start, end in (
    ('source:Private/AlsCharacter.cpp', als/'Private/AlsCharacter.cpp', 505, 526),
    ('source:Private/AlsCharacter_Actions.cpp', als/'Private/AlsCharacter_Actions.cpp', 90, 100),
    ('Engine/Source/Runtime/AnimGraphRuntime/Private/AnimNodes/AnimNode_Slot.cpp',
     ue/'Engine/Source/Runtime/AnimGraphRuntime/Private/AnimNodes/AnimNode_Slot.cpp', 49, 86),
    ('Engine/Source/Runtime/Engine/Private/Animation/AnimInstanceProxy.cpp',
     ue/'Engine/Source/Runtime/Engine/Private/Animation/AnimInstanceProxy.cpp', 2103, 2177),
    ('src/animation_graph_ext/source_slot.rs', Path('src/animation_graph_ext/source_slot.rs'), 67, 136),
    ('src/als/character/actions/roll.rs', Path('src/als/character/actions/roll.rs'), 87, 102),
):
    data = path.read_bytes()
    facts.append(dict(key=key, sha256=hashlib.sha256(data).hexdigest(),
                      excerpts=[dict(start=start, end=end, text='\n'.join(data.decode().splitlines()[start-1:end])+'\n')]))
save('source-facts.json', facts)

decoded = {}
for name in ['ab_als.native.json', 'landing.native.json'] + [
    f'runtime-{label}/{file}.json' for label in runs for file in ('report', 'parameters')
]:
    data = raw(name)
    path = root / name
    archive = path.with_suffix('.json.gz')
    if path.exists():
        archive.write_bytes(gzip.compress(data, compresslevel=6, mtime=0))
    assert gzip.decompress(archive.read_bytes()) == data
    decoded[name] = dict(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
    print('Verified gzip ' + name, flush=True)
paths = [
    target, root.parent/'484/playground_observed.rs', root.parent/'484/foot_observer.rs',
    root.parent/'505/landing.native.json.gz', root.parent/'505/landing.request.json',
    root.parent/'505/observer.py', root.parent/'505/velocity_observer.rs',
    root.parent/'491/reader-migration.json', root.parent/'441c/library-identities.json',
]
external = [dict(path=str(path.resolve().relative_to(Path.cwd())), bytes=path.stat().st_size,
                 sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for path in paths]
save('summary.json', dict(
    gate='PASS_AUTOMATIC_ROLL_GROUNDED_SLOT_OCCLUSION',
    native_landing=25, target_landing=59, aligned_frames=35,
    first_fully_occluded_offset=2, first_updated_again_offset=26, inactive_roll_offset=34,
    masked_source_ablation_updates=24, snap_error_at_target_return=snap_error,
    damp_error_at_target_return=damp_error, roll_fade_numeric_mismatch_offsets=changed,
    maximum_fade_weight_difference=max(abs(row['difference']) for row in comparison),
    restored_parameters_exact=True, screenshots=0, performance_acceptance='paused',
    new_differences=0, total_differences=279, decoded=decoded, external_source_files=external))
