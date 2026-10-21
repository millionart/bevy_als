import json
import math
import sys
from pathlib import Path


def compare_transform(actual, expected):
    for field in ['translation', 'rotation', 'scale']:
        a = list(actual[field].values())
        b = list(expected[field].values())
        assert all(math.isfinite(value) for value in a + b)
        if field == 'rotation' and sum(x * y for x, y in zip(a, b)) < 0:
            b = [-value for value in b]
        assert a == b, (field, a, b)


def check(document):
    frames = document['sequence_pose_probe']['frames']
    controls = 0
    virtuals = 0
    for frame in frames:
        assert frame['pose_evaluation_mode'] == 'raw_model'
        stages = frame['source_stages']
        assert stages['apply_mode'] == 2
        assert len(stages['bones']) == len(frame['bones']) == 79
        names = {bone['name'] for bone in stages['bones']}
        assert names == {bone['name'] for bone in frame['bones']}
        pose = {bone['name']: bone['local_transform'] for bone in frame['bones']}
        for bone in stages['bones']:
            for field in ['bone_local', 'bone_initial_local']:
                assert all(math.isfinite(value) for channel in bone[field].values() for value in channel.values())
            if 'native_channel_control' in bone:
                compare_transform(bone['native_channel_control'], bone['control_local'])
                controls += 1
            if 'retargeted_bone' in bone:
                compare_transform(bone['retargeted_bone'], pose[bone['name']])
        assert len(stages['post_pose_virtual_replay']) == 11
        for replay in stages['post_pose_virtual_replay']:
            compare_transform(replay['relative'], pose[replay['name']])
            virtuals += 1
    assert controls == len(frames) * 68
    return {'frames': len(frames), 'native_channel_controls_exact': controls,
            'native_virtual_replays_exact': virtuals, 'source_mode': 'Direct'}


if __name__ == '__main__':
    document = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))
    print(json.dumps(check(document)))
