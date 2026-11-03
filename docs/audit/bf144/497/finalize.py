import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/497')
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.resolve().relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

for label in ['center','forward','right']:
    assert read(f'{label}.native.json.reader.report.json')['success']
assert read('runtime-observed/process.report.json')['success']
assert not read('runtime-no-cached-clock/process.report.json')['success']
gate = read('observed-functional.json')
assert gate['pass'] and gate['validation_scope']=='functional' and not gate['issues'] and not gate['images']
assert not read('replay-weight-order-assumption.report.json')['success']
for label in ['baseline','restored','no-clock','no-raw-reference']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success']==(label in ['baseline','restored'])
assert (root/'replay-baseline.json').read_bytes()==(root/'replay-restored.json').read_bytes()
result = read('replay-restored.json')
assert result['clock_mismatches']==0 and result['native_witness_alpha']==0.6186039447784424
rows = result['rows']
assert [row['query'] for row in rows]==list(range(99,111))
assert [row['query'] for row in rows if row['output']!=row['raw_output']]==list(range(105,111))
assert [row['actual_alpha'] for row in rows[:6]]==[0.5]*6
assert all(row['raw_output']==row['base'] and row['raw_delta']==[0,0,0] for row in rows)
assert rows[6]['actual_alpha']==0.6186038851737976
clock_negative = read('replay-no-clock.json')
assert clock_negative['clock_mismatches']==6
assert [row['query'] for row in clock_negative['rows'] if row['mapped']!=row['actual_alpha']]==list(range(105,111))
raw_negative = read('replay-no-raw-reference.json')
assert raw_negative['clock_mismatches']==0
assert all(row['output']==row['raw_output'] for row in raw_negative['rows'])
evidence = read('evidence-input.json')
assert [row['query'] for row in evidence['timeline'] if not row['active']]==[95,96,97,98]
assert all(row['alpha']['time_update']=='None' for row in evidence['timeline'] if row['active'])
print('Verified exact interpolation, source witness and reference ablations',flush=True)

decoded = {}
for name in ['center.native.json','forward.native.json','right.native.json',
             'runtime-observed/parameters.json','runtime-observed/report.json']:
    path = root/name
    raw = path.read_bytes()
    packed = gzip.compress(raw,mtime=0)
    assert gzip.decompress(packed)==raw
    path.with_suffix('.json.gz').write_bytes(packed)
    decoded[name]=dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
references = [parent/name for name in ['495/observer.py','496/runtime-observed/parameters.json.gz',
    '496/lean.native.json.gz','493/virtual-motion.native.json.gz','484/playground_observed.rs','484/foot_observer.rs',
    '491/reader-migration.json','441c/library-identities.json']]
references.extend(Path(name+suffix) for name in evidence['assets'].values() for suffix in ['.gltf','.bin'])
references += [Path(evidence['graph']),Path('tools/import/audit_animation_transform_conversion.py')]
summary = dict(gate='PASS_ACTUAL_LEAN_SAMPLE_CLOCK_AND_RAW_REFERENCE_FACTOR',new_differences=0,known_total=276,
               existing_difference='D235',native_leaves=3,native_sample_indices=[0,1,4],
               target_observed_frames=16,target_active_frames=12,first_active_query=99,
               first_affected_scale_query=105,raw_reference_restored_queries=list(range(105,111)),
               target_witness_alpha=rows[6]['actual_alpha'],native_witness_alpha=result['native_witness_alpha'],
               native_source_raw=evidence['native_alpha']['raw'],target_source_raw=evidence['timeline'][10]['alpha']['raw'],
               player_delta_reproduces_target=True,native_alpha_only_preserves_target_witness_scale=True,
               whole_native_parent_weight_history_compared=False,screenshots=0,performance_acceptance='paused',
               decoded=decoded,generated_observer=identity(temporary/'playground_observed.rs'),
               external_source_files=[identity(path) for path in references])
(root/'final-source.json').write_text(json.dumps(identity(root/'replay.rs'),indent=2)+'\n')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
