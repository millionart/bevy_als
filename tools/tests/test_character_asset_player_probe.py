import argparse
import json
import math
from pathlib import Path


def validate(document):
    cases = document['character_animation_probe']['cases']
    assert cases
    players = modes = positive = 0
    for case in cases:
        instances = case['initialization']['instances']
        assert instances
        for instance in instances:
            rows = instance['asset_players']
            assert len({row['property'] for row in rows}) == len(rows)
            for row in rows:
                assert isinstance(row['asset'], str) and row['node_type']
                assert type(row['required_bones_valid']) is bool
                for key in ['current_asset_time', 'cached_blend_weight']:
                    assert type(row[key]) in (int, float) and math.isfinite(row[key])
                if 'pose_mode_at_observation' in row:
                    assert row['asset'] and row['required_bones_valid']
                    assert row['pose_mode_at_observation'] in {'raw', 'compressed'}
                    modes += 1
                for sample in row.get('blend_samples', []):
                    assert type(sample['index']) is int and isinstance(sample['asset'], str)
                    assert all(math.isfinite(sample[key]) for key in ['time', 'cached_time', 'weight'])
                    assert 0 <= sample['weight'] <= 1
                    if 'pose_mode_at_observation' in sample:
                        assert sample['asset'] and row['required_bones_valid']
                        assert sample['pose_mode_at_observation'] in {'raw', 'compressed'}
                positive += row['cached_blend_weight'] > 0
                players += 1
    assert players and modes and positive
    return {'initial_asset_players': players, 'sequence_modes_at_observation': modes, 'positive_cached_weights': positive, 'earlier_evaluation_mode_proven': False}


def self_test():
    row = {'property': 'player', 'node_type': 'sequence', 'asset': 'asset', 'required_bones_valid': True, 'current_asset_time': 0, 'cached_blend_weight': 1, 'pose_mode_at_observation': 'raw'}
    fixture = {'character_animation_probe': {'cases': [{'initialization': {'instances': [{'asset_players': [row]}]}}]}}
    assert validate(fixture)['initial_asset_players'] == 1
    rejected = 0
    for index in range(5):
        changed = json.loads(json.dumps(fixture))
        instance = changed['character_animation_probe']['cases'][0]['initialization']['instances'][0]
        if index == 0:
            instance.pop('asset_players')
        elif index == 1:
            instance['asset_players'][0]['current_asset_time'] = float('nan')
        elif index == 2:
            instance['asset_players'][0]['pose_mode_at_observation'] = 'unknown'
        elif index == 3:
            instance['asset_players'].append(instance['asset_players'][0])
        else:
            instance['asset_players'][0]['blend_samples'] = [{'index': 0, 'asset': 'sample', 'time': .5, 'cached_time': .5, 'weight': -1}]
        try:
            validate(changed)
        except (KeyError, AssertionError):
            rejected += 1
    assert rejected == 5
    return {'self_test': 'PASS', 'invalid_player_observations_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--input', type=Path)
    group.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    print(json.dumps(self_test() if args.self_test else validate(json.loads(args.input.read_text()))))
