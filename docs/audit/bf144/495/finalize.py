import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/495')
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.resolve().relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

for name in ['stand','idle','look']:
    assert read(f'{name}.native.json.reader.report.json')['success']
for name in ['observed','moving']:
    assert read(f'runtime-{name}/process.report.json')['success']
    gate = read(f'{name}-functional.json')
    assert gate['pass'] and gate['validation_scope']=='functional' and not gate['issues'] and not gate['images']
assert not read('compile-cache-option.report.json')['success']
assert not read('prepare-import-path.report.json')['success']
assert not read('prepare-inactive-lean.report.json')['success']
assert not read('replay-zero-delta-channel.report.json')['success']
for label in ['baseline','restored','no-raw-reference']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success']==(label!='no-raw-reference')
assert (root/'replay-baseline.json').read_bytes()==(root/'replay-restored.json').read_bytes()

rows = read('replay-restored.json')
negative = read('replay-no-raw-reference.json')
assert [row['query'] for row in rows]==[0,1,2,3,105,158]
assert [row['query'] for row in rows]==[row['query'] for row in negative]
for row,control in zip(rows,negative,strict=True):
    assert row['controls'][0]==control['controls'][0]
    fields = [key for key in row['controls'][0] if key not in ['raw_idle','raw_look']]
    assert all(all(c[key]==row['controls'][0][key] for key in fields) for c in control['controls'])
    assert row['controls'][1]['idle_delta']==[0,0,0]
    assert row['controls'][2]['look_delta']==[0,0,0]
for index in [2,3]:
    assert [x['head'][1] for x in rows[index]['controls']]==[1.000000238418579,1.0000001192092896,1.0000001192092896,1.0]
motion = read('motion-remaining.json')
assert [row['query'] for row in motion]==[105,158]
assert motion[0]['locomotion'][1]==1.000000238418579
assert motion[0]['lean_additive']==[0,1.1920928955078125e-07,0]
assert motion[1]['lean_additive'] is None
print('Verified exact Idle and Head factor controls and recorded separate motion scales',flush=True)

decoded = {}
for name in ['stand.native.json','idle.native.json','look.native.json',
             *[f'runtime-{label}/{file}.json' for label in ['observed','moving'] for file in ['parameters','report']]]:
    path = root/name
    raw = path.read_bytes()
    packed = gzip.compress(raw,mtime=0)
    assert gzip.decompress(packed)==raw
    path.with_suffix('.json.gz').write_bytes(packed)
    decoded[name]=dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
references = [parent/name for name in ['449/stand-raw.native.json','449/stand-compressed-fixed.native.json',
    '484/playground_observed.rs','484/foot_observer.rs','484/runtime-observed/parameters.json.gz',
    '493/virtual-motion.native.json.gz','491/reader-migration.json','441c/library-identities.json']]
references.extend(Path('assets/ALS/Animations')/(name+suffix)
    for name in ['Base/A_Als_Stand_Pose','Base/A_Als_Idle','View/A_Als_Look_Forward'] for suffix in ['.gltf','.bin'])
references.extend([Path('assets/ALS/Character/AnimationInstances/Overlays/AB_Als_Default/Overlay.animgraph.ron'),
                   Path('tools/import/audit_animation_transform_conversion.py')])
summary = dict(gate='PASS_D235_VIRTUAL_FOOT_SCALE_ORIGIN',new_differences=0,existing_difference='D235',
               known_total=276,native_leaf_samples=5,actual_target_frames=6,
               first_nonuniform_head_query=2,first_head_scale_y=1.000000238418579,
               one_reference_only_head_scale_y=1.0000001192092896,both_raw_references_head_scale_y=1.0,
               moving_upstream_scale_factors_closed=False,screenshots=0,performance_acceptance='paused',
               decoded=decoded,generated_observer=identity(temporary/'playground_observed.rs'),
               external_source_files=[identity(path) for path in references])
(root/'final-source.json').write_text(json.dumps(identity(root/'replay.rs'),indent=2)+'\n')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
