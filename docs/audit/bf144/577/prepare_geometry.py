import gzip
import json
import math
from pathlib import Path

root = Path(__file__).resolve().parent
raw = gzip.decompress((root.parent / '575' / 'lateral.native.json.gz').read_bytes())
source = {case['name']: case['frames'][58] for case in json.loads(raw)['character_animation_probe']['cases']}


def axes(vector, scale):
    return [vector[index] * scale for index in (0, 2, 1)]


samples = []
for label in ('positive', 'negative'):
    live = json.loads((root / f'runtime-{label}' / 'landing-geometry.json').read_bytes())
    assert live['floor_rotation'] == [0, 0, 0, 1]
    assert live['floor_center'][1] + live['floor_half_extents'][1] == 0
    assert abs(live['orientation'][0]) < 1e-7 and abs(live['orientation'][2]) < 1e-7
    size = live['capsule_extents']
    center = axes(live['floor_center'], 100)
    half = axes(live['floor_half_extents'], 100)
    for variant in ('actual', 'direction_control', 'height_control', 'reference_box'):
        row = live['actual' if variant == 'reference_box' else variant]
        box = [-1500, -1500, -10] if variant == 'reference_box' else center
        extent = [2000, 1500, 10] if variant == 'reference_box' else half
        samples.append(dict(name=f'{label}-{variant}', rotation_xyzw=[0, 0, 0, 1], capsule_sweep=dict(
            box_translation_cm=box, box_half_extents_cm=extent,
            start_cm=axes(row['start'], 100), direction=axes(row['direction'], 1),
            radius_cm=size[0]*100, half_height_cm=size[1]*100,
            length_cm=live['max_distance']*100)))
    hit, = source[label]['blocking_hits']
    start = [hit['trace_start_cm'][axis] for axis in 'xyz']
    end = [hit['trace_end_cm'][axis] for axis in 'xyz']
    direction = [end_value - start_value for start_value, end_value in zip(start, end)]
    length = math.hypot(*direction)
    samples.append(dict(name=f'{label}-archived', rotation_xyzw=[0, 0, 0, 1], capsule_sweep=dict(
        box_translation_cm=[-1500,-1500,-210], box_half_extents_cm=[2000,1500,10],
        start_cm=start, direction=[component/length for component in direction],
        radius_cm=30, half_height_cm=90, length_cm=length)))
(root / 'geometry.request.json').write_text(json.dumps(dict(samples=samples), indent=2) + '\n')
print('Prepared live-world, direction, height, and box controls for native Chaos', flush=True)
