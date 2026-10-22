import json
import math
import sys
from pathlib import Path


def check(document, full_pose=None):
    probe = document['sequence_pose_probe']
    assert probe['sampling_mode'] == 'compressed_local_tracks'
    compression = probe['compression']
    mapping = compression['track_to_skeleton']
    assert len(mapping) == len(set(mapping))
    assert compression['key_count'] > 0
    assert compression['sampling_rate_numerator'] > 0
    assert compression['sampling_rate_denominator'] > 0
    assert compression['bone_codec'] and compression['curve_codec']
    if 'indexed_curve_names' in compression:
        indexed = compression['indexed_curve_names']
        curve_names = [item['name'] for item in indexed]
        assert len(curve_names) == len(set(curve_names))
        assert sorted(item['sorted_index'] for item in indexed) == list(range(len(indexed)))
        assert set(curve_names) == set(probe['frames'][0]['codec_curves'])
    expected = None if full_pose is None else full_pose['sequence_pose_probe']['frames']
    if expected is not None:
        assert len(expected) == len(probe['frames'])
    exact = 0
    references = {bone['name']: bone['reference_pose'] for bone in probe['frames'][0]['bones'] if 'reference_pose' in bone}
    for index, frame in enumerate(probe['frames']):
        assert all(math.isfinite(value) for value in frame['codec_curves'].values())
        if 'indexed_curve_names' in compression:
            assert set(curve_names) == set(frame['codec_curves'])
        names = [bone['name'] for bone in frame['bones']]
        assert len(names) == len(set(names))
        assert all(0 <= bone < len(names) for bone in mapping)
        if expected is not None:
            assert frame['time'] == expected[index]['time']
            assert expected[index]['pose_evaluation_mode'] == 'compressed'
            assert frame['codec_curves'] == expected[index]['curves']
            wanted = {bone['name']: bone for bone in expected[index]['bones']}
            assert set(wanted) == set(names)
        for bone in frame['bones']:
            fields = ['local_transform', 'codec_pose', 'retargeted_pose']
            if references:
                assert bone['reference_pose'] == references[bone['name']]
                fields.append('reference_pose')
            for field in fields:
                transform = bone[field]
                assert set(transform) == {'translation', 'rotation', 'scale'}
                for channel, values in transform.items():
                    assert set(values) == set('xyzw' if channel == 'rotation' else 'xyz')
                    assert all(math.isfinite(value) for value in values.values())
            if expected is not None:
                assert bone['parent_index'] == wanted[bone['name']]['parent_index']
                assert bone['retargeted_pose'] == wanted[bone['name']]['local_transform'], (frame['time'], bone['name'])
                exact += 1
    return {'frames': len(probe['frames']), 'native_retargeted_bones_exact': exact,
            'input_key_count': compression['key_count'], 'mapped_tracks': len(mapping)}


if __name__ == '__main__':
    document = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig'))
    expected = None if len(sys.argv) < 3 else json.loads(Path(sys.argv[2]).read_text(encoding='utf-8-sig'))
    print(json.dumps(check(document, expected)))
