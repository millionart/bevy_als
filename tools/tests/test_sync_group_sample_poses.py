import copy
import json
import math
import sys
from pathlib import Path


def check_mesh_contract(document, mesh_path):
    probe = document['sync_group_clock_probe']
    assert type(mesh_path) is str and mesh_path
    assert probe['required_bones_mesh'] == mesh_path
    assert probe['players']
    for player in probe['players']:
        assert player['pose_mesh'] == mesh_path
        assert player['pose_skeleton'] and type(player['pose_bone_count']) is int and player['pose_bone_count'] > 0
    return {'mesh_asset': mesh_path, 'players': len(probe['players'])}


def check(document, original=None, mesh_path=None):
    probe = document['sync_group_clock_probe']
    if mesh_path is not None:
        check_mesh_contract(document, mesh_path)
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


def self_test_mesh():
    mesh = '/Example/Mesh.Mesh'
    fixture = {'sync_group_clock_probe': {'required_bones_mesh': mesh, 'players': [{'pose_mesh': mesh, 'pose_skeleton': '/Example/Skeleton.Skeleton', 'pose_bone_count': 79}]}}
    assert check_mesh_contract(fixture, mesh)['players'] == 1
    rejected = 0
    for index in range(4):
        bad = copy.deepcopy(fixture)
        probe = bad['sync_group_clock_probe']
        if index == 0:
            del probe['required_bones_mesh']
        elif index == 1:
            probe['players'][0]['pose_mesh'] = '/Other/Mesh.Mesh'
        elif index == 2:
            probe['players'][0]['pose_bone_count'] = 0
        else:
            probe['players'] = []
        try:
            check_mesh_contract(bad, mesh)
        except (KeyError, AssertionError):
            rejected += 1
    assert rejected == 4
    return {'self_test': 'PASS', 'invalid_mesh_contracts_rejected': rejected}


if __name__ == '__main__':
    document = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))
    original = None if len(sys.argv) < 3 else json.loads(Path(sys.argv[2]).read_text(encoding='utf-8-sig'))
    print(json.dumps(check(document, original)))
