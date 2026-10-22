import copy
import json
import math
import sys
from pathlib import Path


def check(document, original=None):
    probe = document['sync_group_clock_probe']
    stripped = copy.deepcopy(probe)
    poses = 0
    samples = 0
    for case in stripped['samples']:
        for frame in case['frames']:
            for player in frame['players']:
                player.pop('blend_inner_pose', None)
                for sample in player.get('samples', []):
                    if 'pose' not in sample:
                        continue
                    assert player['ticked'] and sample['pose_evaluation_mode'] == 'compressed'
                    pose = sample.pop('pose')
                    assert len(pose['bones']) == len(player['pose']['bones'])
                    assert [bone['name'] for bone in pose['bones']] == [bone['name'] for bone in player['pose']['bones']]
                    for bone in pose['bones']:
                        for values in bone['local_transform'].values():
                            assert all(math.isfinite(value) for value in values.values())
                    assert all(math.isfinite(value) for value in pose['curves'].values())
                    samples += 1
                if player['ticked'] and player.get('samples'):
                    poses += 1
    assert poses > 0 and samples >= poses
    if original is not None:
        expected = copy.deepcopy(original['sync_group_clock_probe'])
        for case in expected['samples']:
            for frame in case['frames']:
                for player in frame['players']:
                    player.pop('blend_inner_pose', None)
                    for sample in player.get('samples', []):
                        sample.pop('pose', None)
        assert stripped == expected
    return {'blendspace_poses': poses, 'sample_poses': samples, 'original_clock_and_output_exact': original is not None}


if __name__ == '__main__':
    document = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))
    original = None if len(sys.argv) < 3 else json.loads(Path(sys.argv[2]).read_text(encoding='utf-8-sig'))
    print(json.dumps(check(document, original)))
