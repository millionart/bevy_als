import argparse
import gzip
import json
from pathlib import Path

from coordinate_basis import source_point_to_runtime


def point(value):
    return source_point_to_runtime([value['x'], value['y'], value['z']])


def pair(value, prefix):
    return [point(value[f'{prefix}_0_cm']), point(value[f'{prefix}_1_cm'])]


def convert(source):
    samples = []
    for sample in source['physics_contact_probe']['samples']:
        request = sample['input']
        history = request['history']
        current = [pair(value, 'shape_point') for value in sample['points']]
        saved = [pair(value, 'shape_point') for value in sample['saved_points']]
        next_points = [pair(value, 'shape_point') for value in sample['next_points']]
        next_history = sample['next_history']
        if len(current) != len(history['friction_ratios']) or len(next_points) != len(next_history):
            raise ValueError('Native history point counts differ')
        kinds = [sample['geometry']['kind'], sample.get('other_geometry', {}).get('kind', 'box')]
        samples.append({
            'label': request['label'],
            'quadratic': [kind in ('sphere', 'capsule') for kind in kinds],
            'current': current,
            'anchor_offsets': [source_point_to_runtime(history[f'anchor_offset_{index}_cm']) for index in range(2)],
            'ratios': history['friction_ratios'],
            'saved': saved,
            'next_points': next_points,
            'next_matches': [value['has_static_friction_anchor'] for value in next_history],
            'next_anchors': [pair(value, 'shape_anchor') for value in next_history],
        })
    return {'samples': samples}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.source.resolve() == args.output.resolve():
        raise ValueError('Target output must differ from source')
    opener = gzip.open if args.source.suffix == '.gz' else open
    with opener(args.source, 'rt', encoding='utf-8-sig') as source:
        result = convert(json.load(source))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
