import json
import math
import sys
from pathlib import Path


def check(document):
    probe = document['sync_group_clock_probe']
    request = json.loads(Path(__file__).with_name('fixtures').joinpath('sync_group_pose.request.json').read_text())
    assert len(probe['players']) == 2 and len(probe['samples']) == 3
    names = None
    poses = 0
    mixed = False
    for expected, row in zip(request['samples'], probe['samples']):
        assert row['name'] == expected['name'] and len(row['frames']) == len(expected['steps'])
        for step, frame in zip(expected['steps'], row['frames']):
            for index, player in enumerate(frame['players']):
                active = step['weights'][index] is not None
                assert player['ticked'] == active and 'pose' in player
                if not active:
                    assert player['pose'] is None
                    continue
                pose = player['pose']
                bones = pose['bones']
                bone_names = [bone['name'] for bone in bones]
                assert bone_names and len(set(bone_names)) == len(bone_names)
                if names is None:
                    names = bone_names
                assert bone_names == names and len(bones) == probe['players'][index]['pose_bone_count']
                assert probe['players'][index]['pose_skeleton']
                assert all(math.isfinite(value) for value in pose['curves'].values())
                for number, bone in enumerate(bones):
                    assert -1 <= bone['parent_index'] < number
                    transform = bone['local_transform']
                    assert all(math.isfinite(value) for field in transform.values() for value in field.values())
                    norm = sum(value * value for value in transform['rotation'].values())
                    assert abs(norm - 1.0) < 1e-5, (row['name'], bone['name'], norm)
                mixed |= index == 0 and sum(sample['weight'] > 0 for sample in player['samples']) > 1
                poses += 1
    assert poses == 14 and mixed
    return {'frames':12, 'player_poses':poses, 'bones':len(names), 'mixed_blendspace_observed':mixed}


def check_sequence_oracle(document, oracle):
    rows = document['sync_group_clock_probe']['samples']
    frames = oracle['sequence_pose_probe']['frames']
    selected = [(row, index) for row in rows if row['name'] in ['one_hot_blendspace', 'sequence']
                for index in range(len(row['frames']))]
    assert len(selected) == len(frames) == 8
    maximum = 0.0
    checked = 0
    for (row, index), reference in zip(selected, frames):
        player_index = 0 if row['name'] == 'one_hot_blendspace' else 1
        player = row['frames'][index]['players'][player_index]
        mode = player['samples'][0].get('pose_evaluation_mode') if player_index == 0 else player.get('pose_evaluation_mode')
        assert mode == reference.get('pose_evaluation_mode'), (row['name'], mode, reference.get('pose_evaluation_mode'))
        time = player['samples'][0]['time'] if player_index == 0 else player['position']
        assert time == reference['time']
        pose = player['pose']
        assert pose['curves'] == reference['curves']
        assert len(pose['bones']) == len(reference['bones']) == 79
        for bone, expected in zip(pose['bones'], reference['bones']):
            assert bone['name'] == expected['name'] and bone['parent_index'] == expected['parent_index']
            actual = bone['local_transform']
            wanted = expected['local_transform']
            for field in ['translation', 'scale']:
                assert actual[field] == wanted[field], (row['name'], bone['name'], field)
            rotation = actual['rotation']
            target = wanted['rotation']
            if player_index == 1:
                assert rotation == target
            else:
                norm = math.sqrt(sum(v * v for v in target.values()))
                normalized = {k:v/norm for k,v in target.items()}
                sign = 1.0 if sum(rotation[k] * normalized[k] for k in rotation) >= 0.0 else -1.0
                error = max(abs(rotation[k] - sign * normalized[k]) for k in rotation)
                maximum = max(maximum, error)
                assert error < 1e-9, (row['name'], bone['name'], error)
            checked += 1
    return {'oracle_frames':8, 'bone_transforms':checked,
            'single_sample_rotation_normalization_max_component_error':maximum,
            'curves_translation_scale_and_sequence_rotation_exact':True}


if __name__ == '__main__':
    data = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))
    result = check(data)
    if len(sys.argv) == 3:
        result['sequence_oracle'] = check_sequence_oracle(data, json.loads(Path(sys.argv[2]).read_text(encoding='utf-8-sig')))
    print(json.dumps(result))
