import gzip
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
temporary = Path('tools/tmp/bf144/500')
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.resolve().relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

for label in ['baseline','restored','no-history-input']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success']==(label!='no-history-input')
assert (root/'replay-baseline.json').read_bytes()==(root/'replay-restored.json').read_bytes()
actual = read('replay-restored.json')
negative = read('replay-no-history-input.json')
assert actual['actual_mismatches']==0 and negative['actual_mismatches']==22
assert actual['source_models']==negative['source_models']==96
inputs = json.loads((parent/'499/evidence-input.json').read_bytes())
rows = actual['rows']
assert len(rows)==len(negative['rows'])==len(inputs)==11
for row,control,observed in zip(rows,negative['rows'],inputs,strict=True):
    assert row['query']==control['query']==observed['query']
    assert [grid['mask'] for grid in row['grid']]==list(range(8))
    for stage in ['fixed','continuous']:
        assert row['grid'][0]['source'][stage]==observed['source']['actual']['amounts']
        assert row['grid'][7]['target'][stage]==observed['target']['actual']
        assert control['grid'][7]['target'][stage]!=observed['target']['actual']
    for mask in range(8):
        assert control['grid'][mask]['source']==row['grid'][mask&3]['source']
        assert control['grid'][mask]['target']==row['grid'][mask&3]['target']

witnesses = []
for query in [104,106]:
    grid=next(r['grid'] for r in rows if r['query']==query)
    assert grid[0]['source']['fixed']==grid[0]['target']['fixed']
    assert grid[7]['source']['fixed']==grid[7]['target']['fixed']
    assert grid[0]['source']['fixed']!=grid[7]['target']['fixed']
    for index in [0,1,2,4,7]:
        bounds=grid[index]['source']['fixed_limits']
        assert bounds['minimum']==bounds['maximum']
    witnesses.append(dict(query=query,source=grid[0]['source']['fixed'],target=grid[7]['target']['fixed'],
        source_arithmetic_single={name:grid[index]['source']['fixed']
                                  for name,index in [('velocity',1),('rotation',2),('history',4)]}))
q104=next(r['grid'] for r in rows if r['query']==104)
for side in ['source','target']:
    anchor=q104[0][side]['fixed']
    def displacement(mask):
        return sum(abs(a-b) for a,b in zip(anchor,q104[mask][side]['fixed']))
    assert 0<displacement(1)<displacement(2)<displacement(4)
    assert q104[3][side]['continuous']!=q104[7][side]['continuous']
    assert q104[4][side]['continuous']!=q104[7][side]['continuous']
assert inputs[0]['query']==100 and inputs[0]['source']['previous']['amounts']!=inputs[0]['target']['previous']
print('Verified both exact histories, two crossed witnesses and a distinct observed seed effect',flush=True)

decoded = {}
for label in ['baseline','restored','no-history-input']:
    path=root/f'replay-{label}.json'
    raw=path.read_bytes()
    packed=gzip.compress(raw,mtime=0)
    assert gzip.decompress(packed)==raw
    path.with_suffix('.json.gz').write_bytes(packed)
    decoded[path.name]=dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())
references=['499/evidence-input.json','499/replay-restored.json.gz','499/replay.rs',
            '498/summary.json','491/reader-migration.json','441c/library-identities.json']
summary=dict(gate='PASS_CROSSED_VELOCITY_BLEND_VELOCITY_ROTATION_HISTORY',new_differences=0,known_total=277,
             actual_frames_per_side=11,exact_source_models=96,exact_fixed_witness_queries=[104,106],
             fixed_witnesses=witnesses,seed_query=99,
             native_seed=inputs[0]['source']['previous']['amounts'],target_seed=inputs[0]['target']['previous'],
             target_ablation_endpoint_mismatches=22,seed_origin_closed=False,
             native_rounding_limits_recorded=True,screenshots=0,performance_acceptance='paused',
             decoded=decoded,generated_math=identity(temporary/'math.rs'),
             external_source_files=[identity(parent/path) for path in references])
(root/'final-source.json').write_text(json.dumps(identity(root/'replay.rs'),indent=2)+'\n')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
