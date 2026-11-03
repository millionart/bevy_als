import collections
import gzip
import hashlib
import json
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
def raw(name):
    path = root / name
    return path.read_bytes() if path.exists() else gzip.decompress((root/(name+'.gz')).read_bytes())
def read(name):
    return json.loads(raw(name))
def sha(data):
    return hashlib.sha256(data).hexdigest()

baseline = read('replay-unit-baseline.json')
assert raw('replay-unit-baseline.json') == raw('replay-unit-restored.json')
assert raw('replay.rs') == raw('replay.rs.final')
assert raw('inputs.json') == raw('inputs.json.final')
hashes = read('final-source.json')
assert sha(raw('replay.rs')) == hashes['sha256']
assert sha(raw('inputs.json')) == hashes['inputs_sha256']
assert not baseline['failures']
assert len(baseline['source']) == 318 and len(baseline['target']) == 319
assert sum(row['published_bones'] for row in baseline['target']) == 25520
assert sum(row['changed'] for row in baseline['source']) == 17363
assert sum(row['f32_changed'] for row in baseline['source']) == 40
control = baseline['held_root_counterexamples']
assert len(control) == 11552 and all(row['root_f32'] == row['published'] for row in control)
by_bone = collections.Counter(row['bone'] for row in control)
assert by_bone['ik_foot_l'] == 79 and by_bone['ik_foot_r'] == 75
witness = next(row for row in control if row['query'] == 99 and row['bone'] == 'ik_foot_l')
assert witness['published'][3] == 0.7065211534500122 and witness['native_end_normalization'][3] == 0.7065210938453674
maximum = max(abs(a-b) for row in control for a,b in zip(row['published'],row['native_end_normalization']))
assert maximum == 1.1920928955078125e-7
assert read('identity-check.json')['six_libraries_unchanged']
for label in ['baseline','restored']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success']

negative = {}
inputs = read('inputs.json')
for case in ['native-final','held-final','prior-feedback']:
    assert read(f'replay-compile-no-{case}.report.json')['success']
    assert read(f'replay-no-{case}.report.json')['process_exit_code'] == 101
    selection = read(f'negative-selection-{case}.json')
    assert sha(raw(f'replay.no-{case}.rs')) == selection['source_sha256']
    assert sha(raw(f'negative-inputs-{case}.json')) == selection['inputs_sha256']
    selected = read(f'negative-inputs-{case}.json')
    assert selected['source'] == inputs['source'][1:2] and selected['target'] == inputs['target'][:2]
    report = read(f'replay-unit-no-{case}.json')
    stages = collections.Counter(row['stage'] for row in report['failures'])
    if case == 'native-final':
        assert stages == {'source_mesh':47}
    elif case == 'held-final':
        assert not stages and not report['held_root_counterexamples']
    else:
        assert stages == {'target_prior_curves':1}
        item = report['failures'][0]
        assert item['query'] == 1 and item['predicted'][0] == 26 and item['observed']['projected_curve_count'] == 14
    negative[case] = dict(failures=stages,held_differences=len(report['held_root_counterexamples']))
(root/'negative-check.json').write_text(json.dumps(negative,indent=2)+'\n')

f32 = lambda v: struct.pack('f',v)
source_witnesses = []
for row in inputs['source'][1:]:
    for name,value in row['pose'].items():
        q = value['rotation']
        mesh = row['mesh'][name]['rotation']
        if list(map(f32,q)) != list(map(f32,mesh)):
            source_witnesses.append(dict(query=row['query'],bone=name,root=q,mesh=mesh))
assert len(source_witnesses) == 40
initial = inputs['source'][0]
initial_differences = {field:sum(value[field]!=initial['mesh'][name][field] for name,value in initial['pose'].items())
                       for field in ['translation','rotation','scale']}
assert initial_differences['translation'] == 21
(root/'native-published-witnesses.json').write_text(json.dumps(source_witnesses,indent=2)+'\n')
for name, item in read('compression-map.json').items():
    print('Checking persistent evidence '+name,flush=True)
    packed = (root/item['compressed']).read_bytes()
    decoded = gzip.decompress(packed)
    assert len(decoded) == item['raw_bytes'] and sha(decoded) == item['raw_sha256']
    assert sha(packed) == item['compressed_sha256']

external = []
for name in [
    'docs/audit/bf144/455/main.native.json.gz','docs/audit/bf144/457/restored-final.native.json.gz',
    'docs/audit/bf144/458/runtime-restored/parameters.json.gz','docs/audit/bf144/458/summary.json',
    'docs/audit/bf144/459/inputs.json','docs/audit/bf144/459/replay-restored.json.gz',
    'docs/audit/bf144/471/replay.rs','docs/audit/bf144/482/replay-unit-restored.json.gz',
    'docs/audit/bf144/481a/reader-migration.json','docs/audit/bf144/441b/identities.start.json',
    'docs/audit/bf144/441c/library-identities.json','docs/audit/bf144/441b/watchdog.self-test.json',
    'tools/run_ue_source_reader.ps1',
]:
    data = Path(name).read_bytes()
    external.append(dict(path=name,bytes=len(data),sha256=sha(data)))

summary = dict(gate='PASS_RIG_ROOT_AND_MESH_PUBLICATION_AUDIT',
               target_rig_root_pose_frames=319,target_bone_publications=25520,
               native_source_mesh_frames=318,native_source_mesh_transforms=25122,
               native_source_norm_changed=17363,native_source_f32_projection_changed=40,
               initialization_mesh_buffer_distinct=initial_differences,
               source_prior_curve_bundles=318,target_observed_feedback_fields_frames=319,
               target_observed_feedback_fields=['projected_curve_count','projected_standing','projected_grounded','global_standing','global_grounded'],
               held_root_rotation_changes=len(control),held_changes_by_bone=by_bone,
               held_root_max_quaternion_component_difference=maximum,ik_foot_left_witness=witness,
               actual_target_writer_via_production_plugin_finish=True,
               controlled_writer_schedule_without_live_character_feedback=True,
               new_differences=['D276'],confirmed_difference_count=273,
               existing_d245_fixed_in_held_root_input=True,
               full_root_feedback_equivalence_claimed=False,
               next_boundary='Read published IK bones into next-frame foot world targets and separate D276 propagation',
               source_and_inputs_restored_same_target_report_byte_exact=True,
               external_source_files=external,screenshots=0,performance_acceptance='paused')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified native mesh normalization, independent Root transport, public writer and three restored negatives',flush=True)
