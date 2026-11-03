import gzip
import hashlib
import json
import struct
from collections import Counter
from pathlib import Path

root = Path(__file__).resolve().parent
parent = root.parent
def read(name):
    return json.loads((root/name).read_bytes())

def identity(path):
    raw = path.read_bytes()
    return dict(path=path.relative_to(Path.cwd()).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

restored = read('replay-restored.json')
removed = read('replay-no-component-normalization.json')
assert (root/'replay-restored.json').read_bytes() == (root/'replay-baseline.json').read_bytes()
assert restored['source_native_misses']==[]
assert read('replay-baseline-index-assumption.report.json')['success'] is False
for label in ['baseline','restored','no-component-normalization']:
    assert read(f'replay-compile-{label}.report.json')['success']
    assert read(f'replay-{label}.report.json')['success'] == (label!='no-component-normalization')

expected_misses = []
counts = {}
independent = {}
for cohort, length in [('source',634),('target',636)]:
    positive_rows = restored[cohort]
    assert len(positive_rows)==len(removed[cohort])==length
    counts[cohort] = [Counter() for _ in range(4)]
    independent[cohort] = 0
    for row,negative in zip(positive_rows,removed[cohort],strict=True):
        grid = row['grid']['tqs']
        assert negative['grid']['tqs']==[grid[1],grid[1],grid[3],grid[3]]
        assert {k:v for k,v in row.items() if k!='grid'}=={k:v for k,v in negative.items() if k!='grid'}
        comparisons = row['grid']['differing_fields']
        for counter, comparison in zip(counts[cohort], comparisons, strict=True):
            counter.update(key for key,changed in comparison.items() if changed)
        if comparisons[0]['q32'] and not comparisons[2]['q32']:
            independent[cohort]+=1
        if cohort=='source' and (comparisons[0]['q64'] or comparisons[0]['t64']):
            expected_misses.append([row['frame'],row['side']])
    print(f'Validated paired normalization and publication grid for {cohort}',flush=True)

assert removed['source_native_misses']==expected_misses
assert len(expected_misses)==634
assert counts['source']==[{'q64':633,'t64':2},{'q64':634,'q32':4,'t64':4},{'q64':293,'q32':1},{'q64':614,'q32':3,'t64':2}]
assert counts['target']==[{'q64':636,'q32':398,'t64':4},{'q64':636,'q32':344,'t64':4},
                          {'q64':155,'q32':46,'t64':2},{'q64':155,'q32':147,'t64':2}]
assert independent['target']==352
witness = next(row for row in restored['target'] if row['query']==3 and row['side']==0)
a,b,c,d = witness['grid']['tqs']
assert a==c and b==d and a['translation']==b['translation']
def f32(value):
    return struct.unpack('<f',struct.pack('<f',value))[0]
qz = [f32(-t['rotation'][1]) for t in [b,a]]
assert qz==[-0.2382742464542389,-0.2382742315530777]

decoded = {}
for label in ['baseline','restored','no-component-normalization']:
    path = root/f'replay-{label}.json'
    raw = path.read_bytes()
    packed = gzip.compress(raw,mtime=0)
    assert gzip.decompress(packed)==raw
    path.with_suffix('.json.gz').write_bytes(packed)
    decoded[path.name] = dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest())

references = ['485/native-input.json.gz','485/native.rs','485/native.json.gz','484/inputs.json.gz',
              '484/replay-unit-restored.json.gz','483/replay-unit-restored.json.gz','479/replay.rs',
              '478/replay.rs','471/replay.rs','481a/reader-migration.json','441c/library-identities.json']
summary = dict(gate='PASS_INDEPENDENT_COMPONENT_SPACE_FOOT_ROTATION_STAGE',new_differences=1,
               difference='D277',priority='P3',known_total=274,source_socket_rows=634,
               target_public_affine_rows=636,source_negative_misses=len(expected_misses),counts=counts,
               target_q32_component_change_without_d276_output_change=independent['target'],
               witness={'query':3,'side':'left','runtime_qz_without_with_component_normalization':qz},
               actual_affine_and_control_are_separate=True,continuous_feedback_claimed=False,
               decoded=decoded,external_source_files=[identity(parent/path) for path in references],
               screenshots=0,performance_acceptance='paused')
(root/'final-source.json').write_text(json.dumps(identity(root/'replay.rs'),indent=2)+'\n')
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print('Verified scoped D277 and lossless replay bytes',flush=True)
