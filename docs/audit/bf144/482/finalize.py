import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent

def raw(name):
    path = root / name
    return path.read_bytes() if path.exists() else gzip.decompress((root / (name + '.gz')).read_bytes())

def read(name):
    return json.loads(raw(name))

def sha(data):
    return hashlib.sha256(data).hexdigest()

comparison = read('comparison.json')
assert comparison['gate'] == 'PASS_EXACT_FINAL_SOURCE_AND_PUBLIC_POSES_WITH_SAME_INPUT_ARITHMETIC_CONTROLS'
assert len(read('lifetime.json')['frames']) == 666
assert read('lifetime.json')['late_arms_return_before_pose_write']
assert raw('replay-unit-controls.json') == raw('replay-unit-restored.json')
assert raw('replay.rs') == raw('replay.rs.final')
assert raw('inputs.json') == raw('inputs.json.final')
hashes = read('final-source.json')
assert sha(raw('replay.rs')) == hashes['sha256']
assert sha(raw('current_vm.rs')) == hashes['current_vm_sha256']
assert sha(raw('inputs.json')) == hashes['inputs_sha256']
for label in ['baseline', 'controls', 'restored']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success']
assert read('current-vm-baseline.report.json')['success']
assert read('current_inputs.report.json')['success']
assert read('identity-check.json')['six_libraries_unchanged']
inputs = read('inputs.json')
names = inputs['names']
negative = {}
for case in ['setter', 'local']:
    assert read(f'replay-compile-no-{case}.report.json')['success']
    assert read(f'replay-no-{case}.report.json')['process_exit_code'] == 101
    selection = read(f'negative-selection-{case}.json')
    assert sha(raw(f'replay.no-{case}.rs')) == selection['source_sha256']
    assert sha(raw(f'negative-inputs-{case}.json')) == selection['inputs_sha256']
    selected = read(f'negative-inputs-{case}.json')
    for domain in ['source', 'current']:
        assert len(selected[domain]) == 1 and selected[domain][0] in inputs[domain]
    report = read(f'replay-unit-no-{case}.json')
    rows = {row['domain']: row['result'] for row in report['rows']}
    if case == 'setter':
        assert report['failures'] == 1 and not rows['source']['foot_exact']
        assert not rows['current']['differences']
        assert {(d['space'],names[d['index']]) for d in rows['source']['differences']} == {
            ('local', 'foot_r'), ('global', 'foot_r'), ('global', 'ball_r')}
    else:
        assert report['failures'] == 2
        assert all(row['foot_exact'] and row['differences'] for row in rows.values())
        assert all(d['space'] == 'local' for row in rows.values() for d in row['differences'])
    negative[case] = {domain: [(d['space'],names[d['index']]) for d in row['differences']] for domain,row in rows.items()}
del inputs
(root/'negative-check.json').write_text(json.dumps(negative, indent=2)+'\n')

for name, item in read('compression-map.json').items():
    print('Checking persistent evidence '+name, flush=True)
    packed = (root/item['compressed']).read_bytes()
    decoded = gzip.decompress(packed)
    assert len(decoded) == item['raw_bytes'] and sha(decoded) == item['raw_sha256']
    assert sha(packed) == item['compressed_sha256']
external = []
for name in [
    'docs/audit/bf144/473/inputs.json','docs/audit/bf144/473/lifetime.json','docs/audit/bf144/473/replay.rs',
    'docs/audit/bf144/476/inputs.json','docs/audit/bf144/476/rig.native.json.gz',
    'docs/audit/bf144/476/actual-after.native.json.gz','docs/audit/bf144/476/weights-after.native.json.gz',
    'docs/audit/bf144/481/inputs.json.gz','docs/audit/bf144/481/replay.rs',
    'docs/audit/bf144/481/current_vm.rs','docs/audit/bf144/481/current_inputs.py',
    'docs/audit/bf144/481/replay-clean-control.json.gz','docs/audit/bf144/481a/reader-migration.json',
    'docs/audit/bf144/441b/identities.start.json','docs/audit/bf144/441c/library-identities.json',
    'docs/audit/bf144/441b/watchdog.self-test.json','tools/run_ue_source_reader.ps1',
    'tools/animgraph_authoring/ue_control_rig_contract.py','tools/animgraph_authoring/ue_control_rig_target.py',
]:
    data = Path(name).read_bytes()
    external.append(dict(path=name,bytes=len(data),sha256=sha(data)))

summary = dict(comparison)
summary.update(gate='PASS_FINAL_FOOT_BALL_AND_FULL_POSE_CONSUMERS',
               native_source_transforms_exact=666*79*2, public_target_transforms_exact=636*79*2,
               same_target_full_report_restored_byte_exact=True, external_source_files=external,
               known_difference_consumers=['D247 ball global translation', 'D232 local unit phase'],
               new_differences=[], confirmed_difference_count=272,
               native_source_setter_foot_and_full_pose_proved=True,
               native_source_whole_raw_setter_snapshot_claimed=False,
               full_rig_or_root_equivalence_claimed=False,
               next_boundary='Rig final local output transfer into the enclosing animation graph and original Root consumers',
               screenshots=0, performance_acceptance='paused')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Qualified exact final poses, held-input arithmetic, two focused negatives and full restoration', flush=True)
