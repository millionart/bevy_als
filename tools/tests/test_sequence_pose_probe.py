import json
import math
import sys
from pathlib import Path


def check(document):
    probe = document['sequence_pose_probe']
    frames = probe['frames']
    assert frames
    expected_names = [bone['name'] for bone in frames[0]['bones']]
    assert expected_names and len(set(expected_names)) == len(expected_names)
    for frame in frames:
        assert 0.0 <= frame['time'] <= probe['duration']
        assert [bone['name'] for bone in frame['bones']] == expected_names
        assert all(math.isfinite(value) for value in frame['curves'].values())
        for index, bone in enumerate(frame['bones']):
            assert -1 <= bone['parent_index'] < index
            transform = bone['local_transform']
            assert all(math.isfinite(value) for field in transform.values() for value in field.values())
            norm = sum(value * value for value in transform['rotation'].values())
            assert abs(norm - 1.0) < 1e-5, (frame['time'], bone['name'], norm)
    assert probe['sampling_mode'] == 'animation_pose'
    return {'frames': len(frames), 'bones': len(expected_names), 'curve_counts': [len(frame['curves']) for frame in frames]}


if __name__ == '__main__':
    print(json.dumps(check(json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig')))))
