import argparse
import json
from pathlib import Path


def validate(document):
    probe = document['character_animation_probe']
    control = probe['raw_data_control']
    assert type(control['requested']) is bool
    assert control['stage'] == 'before_world_creation'
    assert control['applied_value'] == int(control['requested'])
    assert control['value_after_worlds'] == control['applied_value']
    assert control['restored_value'] == control['previous_value']
    assert control['restored_flags'] == control['previous_flags']
    modes = []
    for case in probe['cases']:
        for instance in case['initialization']['instances']:
            for player in instance['asset_players']:
                if player['cached_blend_weight'] <= 0:
                    continue
                if 'pose_mode_at_observation' in player:
                    modes.append(player['pose_mode_at_observation'])
                for sample in player.get('blend_samples', []):
                    if sample['weight'] > 0:
                        modes.append(sample['pose_mode_at_observation'])
    assert modes
    if control['requested']:
        assert set(modes) == {'raw'}
    return {'requested': control['requested'], 'positive_sequence_modes': len(modes), 'borrowed_value_and_flags_restored': True, 'force_disabled_is_not_compressed_proof': True}


def self_test():
    control = dict(requested=True, stage='before_world_creation', applied_value=1, value_after_worlds=1, restored_value=0, previous_value=0, restored_flags=1, previous_flags=1)
    player = dict(cached_blend_weight=1, pose_mode_at_observation='raw')
    fixture = {'character_animation_probe': {'raw_data_control': control, 'cases': [{'initialization': {'instances': [{'asset_players': [player]}]}}]}}
    assert validate(fixture)['borrowed_value_and_flags_restored']
    rejected = 0
    for index in range(5):
        changed = json.loads(json.dumps(fixture))
        value = changed['character_animation_probe']['raw_data_control']
        if index == 0:
            value['requested'] = 1
        elif index == 1:
            value['stage'] = 'after_evaluation'
        elif index == 2:
            value['applied_value'] = 0
        elif index == 3:
            value['restored_flags'] = 2
        else:
            changed['character_animation_probe']['cases'][0]['initialization']['instances'][0]['asset_players'][0]['pose_mode_at_observation'] = 'compressed'
        try:
            validate(changed)
        except AssertionError:
            rejected += 1
    assert rejected == 5
    return {'self_test': 'PASS', 'invalid_controls_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--input', type=Path)
    group.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    print(json.dumps(self_test() if args.self_test else validate(json.loads(args.input.read_text()))))
