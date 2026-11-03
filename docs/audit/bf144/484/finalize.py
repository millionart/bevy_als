import collections
import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
def raw(name):
    path = root/name
    return path.read_bytes() if path.exists() else gzip.decompress((root/(name+'.gz')).read_bytes())
def read(name):
    return json.loads(raw(name))
def sha(data):
    return hashlib.sha256(data).hexdigest()

check = read('input-check.json')
assert check['old_full_trajectory_exact'] and check['valid_frames'] == 318
assert check['prior_chain_nodes'] == 2544 and check['rig_input_fields'] == 1272
assert read('runtime-observed/process.report.json')['success']
functional = read('observed-functional.json')
assert functional['pass'] and functional['validation_scope'] == 'functional' and not functional['images']
assert read('compile-observed.report.json')['success']
assert read('identity-check.json')['six_libraries_unchanged']
assert raw('replay-unit-baseline.json') == raw('replay-unit-restored.json')
assert raw('inputs.json') == raw('inputs.json.final')
assert raw('replay.rs') == raw('replay.rs.final')
identities = read('final-source.json')
for key,name in [('sha256','replay.rs'),('inputs_sha256','inputs.json'),
                 ('observer_sha256','playground_observed.rs'),('foot_observer_sha256','foot_observer.rs')]:
    assert sha(raw(name)) == identities[key]
for label in ['baseline','restored']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success']

negative = {}
for case in ['prime','normalization','current-root']:
    assert read(f'replay-compile-no-{case}.report.json')['success']
    assert read(f'replay-no-{case}.report.json')['process_exit_code'] == 101
    result = read(f'replay-unit-no-{case}.json')
    fields = collections.Counter(item['field'] for item in result['failures'])
    if case == 'prime':
        assert fields == {'states':6,'rig':6}
        assert sorted({item['query'] for item in result['failures']}) == [146,152,163,246,252,263]
    else:
        selection = read(f'negative-selection-{case}.json')
        assert sha(raw(f'replay.no-{case}.rs')) == selection['source_sha256']
        assert sha(raw(f'negative-inputs-{case}.json')) == selection['inputs_sha256']
        assert len(result['rows']) == 1 and result['rows'][0]['query'] == 100
        if case == 'normalization':
            assert not fields and result['rows'][0]['baseline'] == result['rows'][0]['corrected']
        else:
            assert fields == {'states':1,'rig':1,'pelvis':1,'targets':1}
    negative[case] = dict(fields=fields,failed_frames=sorted({item['query'] for item in result['failures']}))
(root/'negative-check.json').write_text(json.dumps(negative,indent=2)+'\n')
for name,item in read('compression-map.json').items():
    print('Checking persistent evidence '+name,flush=True)
    packed = (root/item['compressed']).read_bytes()
    decoded = gzip.decompress(packed)
    assert len(decoded) == item['raw_bytes'] and sha(decoded) == item['raw_sha256']
    assert sha(packed) == item['compressed_sha256']

external = []
for name in [
    'docs/audit/bf144/458/runtime-restored/parameters.json.gz','docs/audit/bf144/458/playground_observed.rs',
    'docs/audit/bf144/458/rustc-args.json','docs/audit/bf144/459/inputs.json',
    'docs/audit/bf144/483/replay-unit-restored.json.gz','docs/audit/bf144/483/summary.json',
    'docs/audit/bf144/481a/reader-migration.json','docs/audit/bf144/441b/identities.start.json',
    'docs/audit/bf144/441c/library-identities.json','docs/audit/bf144/441b/watchdog.self-test.json',
    'tools/run_ue_source_reader.ps1','tools/validate_animation_visual_evidence.py',
    'assets/ALS/Character/CR_Als.runtime.json',
]:
    data = Path(name).read_bytes()
    external.append(dict(path=name,bytes=len(data),sha256=sha(data)))
summary = read('comparison.json')
summary.update(gate='PASS_D276_PUBLISHED_FOOT_WORLD_AND_LOCK_FEEDBACK',
               source_to_target_differences_added=[],new_differences=[],confirmed_difference_count=273,
               functional_original_playground_passed=True,actual_rig_input_fields_exact=1272,
               public_plugin_replayed_under_held_prior_lock_state=True,
               source_socket_contract_read=True,native_socket_value_equivalence_claimed=False,
               source_and_inputs_restored_same_target_report_byte_exact=True,
               external_source_files=external,screenshots=0,performance_acceptance='paused',
               next_boundary='Audit source mesh-to-socket feedback and trace the verified RigInput differences through the original VM')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified scene, public foot lock, RigInput propagation and three negative controls',flush=True)
