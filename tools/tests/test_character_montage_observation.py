import argparse
import copy
import json
import math
from pathlib import Path


def finite(value):
    assert type(value) in [int, float] and math.isfinite(value)


def validate(document):
    packets = 0
    records = 0
    segments = 0
    for case in document['character_animation_probe']['cases']:
        for instances in [case['initialization']['instances']] + [row['linked_instances'] for row in case['frames']]:
            mains = [instance for instance in instances if instance['main']]
            assert len(mains) == 1
            data = mains[0]['montage_evaluation']
            assert isinstance(data, list)
            for montage in data:
                assert isinstance(montage['is_playing'], bool) and isinstance(montage['is_active'], bool)
                assert isinstance(montage['active_blend_profile'], str)
                for key in ['position', 'previous_position', 'delta', 'blend_alpha', 'blend_value', 'blend_start_alpha', 'blend_time', 'blend_time_remaining', 'blend_begin_value', 'blend_desired_value', 'play_length']:
                    finite(montage[key])
                assert type(montage['blend_option']) is int
                assert montage['tracks']
                for track in montage['tracks']:
                    assert isinstance(track['slot_name'], str) and track['slot_name']
                    assert track['segments']
                    for segment in track['segments']:
                        assert isinstance(segment['sequence'], str) and segment['sequence']
                        for key in ['track_start', 'source_start', 'source_end', 'play_rate']:
                            finite(segment[key])
                        assert type(segment['loop_count']) is int and segment['loop_count'] > 0
                        segments += 1
                records += 1
            packets += 1
    assert packets > 0 and records > 0
    return {'complete_main_montage_packets': packets, 'actual_evaluation_records': records, 'actual_source_segment_records': segments}


def self_test():
    montage = {key: 0 for key in ['position', 'previous_position', 'delta', 'blend_alpha', 'blend_value', 'blend_start_alpha', 'blend_time', 'blend_time_remaining', 'blend_begin_value', 'blend_desired_value', 'play_length', 'blend_option']}
    montage.update({'is_playing': True, 'is_active': True, 'active_blend_profile': 'None', 'tracks': [{'slot_name': 'Transition', 'segments': [{'sequence': '/Source', 'track_start': 0, 'source_start': 0, 'source_end': 1, 'play_rate': 1, 'loop_count': 1}]}]})
    instances = [{'main': True, 'montage_evaluation': [montage]}]
    fixture = {'character_animation_probe': {'cases': [{'initialization': {'instances': instances}, 'frames': [{'linked_instances': instances}]}]}}
    assert validate(fixture)['actual_evaluation_records'] == 2
    rejected = 0
    for name in ['montage_evaluation', 'position', 'blend_value', 'tracks', 'sequence', 'nonfinite']:
        data = copy.deepcopy(fixture)
        main = data['character_animation_probe']['cases'][0]['frames'][0]['linked_instances'][0]
        if name == 'montage_evaluation':
            main.pop(name)
        elif name == 'sequence':
            main['montage_evaluation'][0]['tracks'][0]['segments'][0].pop(name)
        elif name == 'nonfinite':
            main['montage_evaluation'][0]['delta'] = float('nan')
        else:
            main['montage_evaluation'][0].pop(name)
        try:
            validate(data)
        except (AssertionError, KeyError):
            rejected += 1
    assert rejected == 6
    return {'self_test': 'PASS', 'missing_or_nonfinite_actual_montage_data_rejected': rejected}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--input', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    print(json.dumps(self_test() if args.self_test else validate(json.loads(args.input.read_text()))))
