import json
import math
import struct
import sys
from pathlib import Path


def without_generation_signatures(document):
    result = json.loads(json.dumps(document))
    classes = {'/Script/AnimationData.AnimationSequencerDataModel', '/Script/MovieScene.MovieScene',
               '/Script/ControlRig.MovieSceneControlRigParameterTrack',
               '/Script/ControlRig.MovieSceneControlRigParameterSection'}
    removed = 0
    for obj in result['asset_objects']:
        if obj['class_path'] not in classes:
            continue
        properties = []
        for prop in obj['properties']:
            if prop['name'] == 'Signature':
                assert prop['cpp_type'] == 'FGuid'
                removed += 1
            else:
                properties.append(prop)
        obj['properties'] = properties
    assert removed == 4
    return result


def check(document, baseline=None):
    source = document['sequence_pose_probe']['source_channels']
    assert source['model'].startswith(document['asset_path'] + ':')
    assert len(source['sections']) == 1
    section = source['sections'][0]
    assert section['active'] and section['path'].startswith(source['model'] + '.')
    assert len(section['transforms']) == 68 and section['scalars']
    channels = [item['channel'] for item in section['scalars']]
    for parameter in section['transforms']:
        assert parameter['name']
        for field in ['translation', 'rotation', 'scale']:
            assert len(parameter[field]) == 3
            channels.extend(parameter[field])
    keys = 0
    precise = 0
    for channel in channels:
        assert channel['tick_numerator'] == 30 and channel['tick_denominator'] == 1
        assert 'pre_infinity' in channel and 'post_infinity' in channel
        values = [] if channel['default'] is None else [channel['default']]
        previous = None
        for key in channel['keys']:
            assert isinstance(key['frame'], int) and (previous is None or key['frame'] >= previous)
            previous = key['frame']
            keys += 1
            values.extend(key[field] for field in ['value', 'arrive_tangent', 'leave_tangent',
                                                 'arrive_weight', 'leave_weight'])
        for value in values:
            assert math.isfinite(value)
            assert struct.unpack('f', struct.pack('f', value))[0] == value
            precise += value != float(format(value, '.6f'))
    assert keys and precise
    if baseline is not None:
        stripped = json.loads(json.dumps(document))
        del stripped['sequence_pose_probe']['source_channels']
        assert without_generation_signatures(stripped) == without_generation_signatures(baseline)
    return {'bone_parameters': 68, 'scalar_parameters': len(section['scalars']),
            'channels': len(channels), 'keys': keys, 'values_beyond_six_decimal_text': precise,
            'original_data_preserved_except_generation_signatures': baseline is not None}


if __name__ == '__main__':
    read = lambda path: json.loads(Path(path).read_text(encoding='utf-8-sig'))
    print(json.dumps(check(read(sys.argv[1]), read(sys.argv[2]) if len(sys.argv) == 3 else None)))
