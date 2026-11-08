import json
import struct
from pathlib import Path


def project(value, scale=1.0):
    return struct.unpack('<f', struct.pack('<f', value * scale))[0]


def main():
    root = Path('docs/audit/bf144/d257')
    probe = json.loads((root / 'right-pose.native.json').read_text())['sequence_pose_probe']
    frames = probe['frames']
    if len(frames) != 2 or any(not frame['compressed_data_valid'] for frame in frames):
        raise ValueError('Expected two valid compressed source poses')
    first, second = frames
    if [bone['name'] for bone in first['bones']] != [bone['name'] for bone in second['bones']]:
        raise ValueError('Source skeleton order changed')
    changed = {'translation': [], 'rotation': [], 'scale': []}
    for a, b in zip(first['bones'], second['bones']):
        for field in changed:
            left = a['local_transform'][field]
            right = b['local_transform'][field]
            scale = 0.01 if field == 'translation' else 1.0
            if [project(left[key], scale) for key in left] != [project(right[key], scale) for key in right]:
                changed[field].append(a['name'])
    curves_changed = sorted(name for name in first['curves'] if project(first['curves'][name]) != project(second['curves'][name]))
    result = {'times': [frame['time'] for frame in frames], 'bones': len(first['bones']),
              'changed': changed, 'changed_counts': {field: len(names) for field, names in changed.items()},
              'curves_changed': curves_changed}
    (root / 'pose-comparison.json').write_text(json.dumps(result, indent=2) + '\n')
    print(f"Compared {len(first['bones'])} source bones", flush=True)


if __name__ == '__main__':
    main()
