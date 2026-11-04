import gzip
import hashlib
import json
import struct
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/502')
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.resolve().relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

for label in ['baseline','restored','no-meter-stage','no-wide-rotation']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success']==(label in ['baseline','restored'])
assert (root/'replay-baseline.json').read_bytes()==(root/'replay-restored.json').read_bytes()
rows = read('replay-restored.json')
no_meters = read('replay-no-meter-stage.json')
no_wide = read('replay-no-wide-rotation.json')
assert [row['query'] for row in rows]==list(range(99,111))
unit_queries = []
widened_queries = []
actual_widened_queries = []
for row,meters,wide in zip(rows,no_meters,no_wide,strict=True):
    assert row['query']==meters['query']==wide['query']
    assert row['runtime_combined']==meters['runtime_combined']==wide['runtime_combined']
    assert row['actual_target']==meters['actual_target']==wide['actual_target']==wide['actual_target_widened']
    assert len(row['reports'])==len(meters['reports'])==len(wide['reports'])==96
    units,precision = [],[]
    for observed,no_meter,no_precision in zip(row['reports'],meters['reports'],wide['reports'],strict=True):
        assert observed['mode']==no_meter['mode']==no_precision['mode']
        grid=observed['grid']
        for index,cell in enumerate(no_meter['grid']):
            assert cell['mask']==index
            assert {k:v for k,v in cell.items() if k!='mask'}=={k:v for k,v in grid[index&3].items() if k!='mask'}
        assert no_precision['expanded64']==observed['runtime32']
        assert no_precision['expanded64_local']==observed['runtime32_local']
        assert observed['expanded64']==observed['cross64']
        units.append(grid[4]['seed']!=observed['native'])
        precision.append(observed['runtime32']!=observed['expanded64'])
    assert len(set(units))==len(set(precision))==1
    if units[0]: unit_queries.append(row['query'])
    if precision[0]: widened_queries.append(row['query'])
    if row['actual_target']!=row['actual_target_widened']:
        actual_widened_queries.append(row['query'])
assert unit_queries==[99,101,102,110]
assert widened_queries==[100,102]
assert actual_widened_queries==[101]
first=rows[0]
assert first['runtime_combined']==[0.10747472196817398,0,0,0.09887273609638214]
assert all(r['grid'][4]['seed']==r['meter_after_native_cast']==first['runtime_combined'] for r in first['reports'])
assert all(r['native']==[0.10747472196817398,0,0,0.09887274354696274] for r in first['reports'])
witness=rows[2]
assert witness['actual_target'][3]==0.21511630713939667
assert witness['actual_target_widened'][3]==0.21511632204055786
def bits(value):
    return struct.unpack('<I',struct.pack('<f',value))[0]
witness_steps=bits(witness['actual_target_widened'][3])-bits(witness['actual_target'][3])
assert witness_steps==1

print('Verified isolated unit and rotation precision factors and independent ablations',flush=True)

decoded = {}
for label in ['baseline','restored','no-meter-stage','no-wide-rotation']:
    path = root/f'replay-{label}.json'
    raw = path.read_bytes()
    packed = gzip.compress(raw,mtime=0)
    assert gzip.decompress(packed)==raw
    path.with_suffix('.json.gz').write_bytes(packed)
    decoded[path.name]=dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
references=['501/evidence-input.json','501/replay-restored.json','499/evidence-input.json',
            '499/replay-restored.json.gz','499/replay.rs','500/replay.rs','491/reader-migration.json',
            '441c/library-identities.json']
summary=dict(gate='PASS_VELOCITY_BLEND_LOCAL_UNIT_AND_ROTATION_PRECISION',new_differences=1,difference='D282',
             priority='P3',known_total=279,existing_unit_difference='D268',
             exact_source_and_target_input_frames=12,exact_native_calibrated_modes=96,
             meter_projection_changes=unit_queries,held_formula_projected_source_changes=widened_queries,
             held_formula_actual_target_changes=actual_widened_queries,
             first_native_right=first['reports'][0]['native'][3],first_meter_right=first['runtime_combined'][3],
             first_runtime_combined_exact=True,wide_formula_switch_final_changes=0,
             witness={'query':101,'observed_right':witness['actual_target'][3],'widened_right':witness['actual_target_widened'][3],'f32_steps':witness_steps},
             screenshots=0,performance_acceptance='paused',decoded=decoded,
             generated_sources=[identity(temporary/name) for name in ['math.rs','basis.rs']],
             external_source_files=[identity(parent/path) for path in references])
(root/'final-source.json').write_text(json.dumps(identity(root/'replay.rs'),indent=2)+'\n')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
