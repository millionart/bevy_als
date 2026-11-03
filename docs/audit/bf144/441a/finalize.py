import gzip
import hashlib
import importlib.util
import json
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parent
validator_path = Path('tools/tests/test_vector_curve_probe.py')
spec = importlib.util.spec_from_file_location('vector_contract', validator_path)
validator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validator)

def read(name):
    return json.loads((root/name).read_bytes())

def native(label):
    return read(f'{label}.native.json')

def probe(label):
    return native(label)['blend_profile_probe']

def status(label, expected):
    result = read(f'{label}.native.json.reader.report.json')
    assert result['process_exit_code'] == expected and result['success'] == (expected == 0), label

assert validator.self_test()['invalid_vector_axis_contracts_rejected'] == 7
request = read('vector.request.json')
restored = native('vector-restored')
assert validator.validate(restored, request)['axes'] == 3
baseline = probe('vector-baseline')
assert restored['blend_profile_probe'] == baseline
assert (root/'vector-restored.native.json').read_bytes() == (root/'vector-baseline.native.json').read_bytes()
rows = baseline['samples']
assert [len(row['custom_curve_keys']) for row in rows] == [4,4,4]
assert [len(row['custom_curve_time_samples']) for row in rows] == [61,61,61]
prior = json.loads((root.parent/'441b/curves.native.json').read_bytes())['blend_profile_probe']['samples']
common_samples = 0
for old, new in zip(prior, rows):
    assert new['custom_curve_keys'] == old['custom_curve_keys']
    old_samples = {s['time']:s['value'] for s in old['custom_curve_time_samples']}
    new_samples = {s['time']:s['value'] for s in new['custom_curve_time_samples']}
    common = old_samples.keys() & new_samples.keys()
    assert common and all(old_samples[t]==new_samples[t] for t in common)
    common_samples += len(common)

ablated = probe('vector-axis-ablated')['samples']
assert len(ablated)==5 and ablated[0] == rows[0]
affected = []
for index in [1,2]:
    control, actual = ablated[index], rows[index]
    data = {'custom_curve_keys','custom_curve_time_samples','custom_curve_pre_extrap','custom_curve_post_extrap'}
    assert {k:v for k,v in control.items() if k not in data} == {k:v for k,v in actual.items() if k not in data}
    assert control['custom_curve_keys'] == rows[0]['custom_curve_keys'] != actual['custom_curve_keys']
    assert control['custom_curve_time_samples'] == rows[0]['custom_curve_time_samples']
    changed = sum(a != b for a,b in zip(control['custom_curve_time_samples'], actual['custom_curve_time_samples']))
    assert changed > 0
    affected.append(changed)
print('Verified exact restored keys and samples and isolated axis ablation',flush=True)

string = probe('string-guard-ablated')['samples']
assert len(string)==1 and string[0]==rows[1]
for label in ['vector-baseline','vector-axis-ablated','vector-restored','float-baseline','float-restored',
              'character-baseline','character-restored','string-guard-ablated']:
    status(label,0)
for label in ['string','range','fraction']:
    status(label+'-rejected',7)
    assert not (root/f'{label}-rejected.native.json').exists()

legacy = probe('float-restored')
assert legacy == probe('float-baseline')
assert (root/'float-restored.native.json').read_bytes() == (root/'float-baseline.native.json').read_bytes()
assert ablated[3:] == legacy['samples']
assert [row['name'] for row in legacy['samples']] == ['Walk','Run']
for row in legacy['samples']:
    assert len(row['custom_curve_time_samples'])==38 and row['custom_curve_keys'] and row['frames']
    assert all(set(key)=={'time','value','arrive_tangent','leave_tangent'} for key in row['custom_curve_keys'])
    assert 'custom_curve_axis' not in row
character_baseline, character_restored = [native(f'character-{mode}') for mode in ['baseline','restored']]
character_probe = character_restored['character_animation_probe']
assert character_baseline['character_animation_probe'] == character_probe, 'Natural character compatibility'
assert len(character_probe['cases'][0]['frames']) == 240
outer_differences = sorted(key for key in character_baseline if character_baseline[key]!=character_restored[key])
assert set(outer_differences) <= {'asset_objects','graphs'}
del character_baseline, character_restored, character_probe
print('Verified native type rejections and full legacy Float and natural 240 probe equality',flush=True)

reader = 'tools/ue_graph_source_reader/Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp'
canonical = Path(reader).read_bytes()
assert canonical == subprocess.check_output(['git','show',f'HEAD:{reader}'],timeout=5)
source = hashlib.sha256(canonical).hexdigest()
assert source == read('source-restore.json')['sha256']
for label in ['baseline','axis','guard','restored']:
    package = read(f'package-{label}.json')
    wanted = read(f'source-{label}.json')['sha256'] if label in ['axis','guard'] else source
    assert package['source_sha256'] == package['package_source_sha256'] == wanted
    if label != 'baseline':
        assert read(f'build-{label}.report.json')['success']
assert read('package-axis.json')['package_binary_sha256'] != read('package-restored.json')['package_binary_sha256']
assert read('package-guard.json')['package_binary_sha256'] != read('package-restored.json')['package_binary_sha256']

def identity(raw):
    return {'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest()}

forbidden = [('/'+part+'/').encode() for part in ('Users','Volumes')]
decoded = {}
for path in sorted(root.glob('*.native.json')):
    raw = path.read_bytes()
    assert all(term not in raw for term in forbidden), path.name
    packed = gzip.compress(raw,mtime=0)
    assert gzip.decompress(packed) == raw
    path.with_suffix('.json.gz').write_bytes(packed)
    decoded[path.name] = identity(raw)
    print('Verified raw roundtrip '+path.name,flush=True)

references = [validator_path, Path('tools/tests/fixtures/vector_curve.request.json'),
              Path('docs/audit/bf144/441b/character.request.json'),Path('docs/audit/bf144/441b/curves.native.json'),
              Path(reader)]
summary = {'gate':'PASS_CURRENT_CURVEVECTOR_ABLATION_RESTORE_AND_LEGACY_COMPATIBILITY',
           'original_probe_commit':'ee52e16a','original_reports_and_identity_snapshot':'missing',
           'reader_sha256':source,'axes':3,'keys':12,'native_samples':183,'previous_common_samples_exact':common_samples,
           'forced_axis_zero_changed_samples':affected,'old_float_curves':2,'old_float_samples_per_curve':38,
           'natural_character_frames':240,'uncompared_character_export_differences':outer_differences,
           'schema_self_test_negatives':7,'native_rejections':3,
           'decoded':decoded,'external_source_files':[{'path':p.as_posix(),**identity(p.read_bytes())} for p in references],
           'screenshots':0,'performance_acceptance':'paused'}
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
