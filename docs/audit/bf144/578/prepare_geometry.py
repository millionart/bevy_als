import json
from pathlib import Path

root = Path(__file__).resolve().parent
rows = json.loads((root / 'runtime-baseline/brake-replay.json').read_text())['steps']
samples = []


def axes(vector):
    return [vector[index]*100 for index in (0,2,1)]


for row in rows:
    if row['empty']:
        continue
    assert row['box_rotation'] == [0, 0, 0, 1]
    for variant in ('full', 'short'):
        capsule = row[f'{variant}_capsule']
        samples.append(dict(name=f'{row["index"]}-{variant}', rotation_xyzw=[0,0,0,1], capsule_sweep=dict(
            box_translation_cm=axes(row['box_center']), box_half_extents_cm=axes(row['box_extents']),
            start_cm=axes(row[f'{variant}_start']), direction=[0,0,-1], radius_cm=capsule[0]*100,
            half_height_cm=capsule[1]*100, length_cm=row[f'{variant}_limit']*100)))
(root / 'geometry.request.json').write_text(json.dumps(dict(samples=samples),indent=2)+'\n')
print('Prepared source full and shrunken same-input floor sweeps', flush=True)
