import copy
import json
import math
import struct
import sys
from pathlib import Path

def validate(data):
    counts = {'fields': 0, 'array_elements': 0}

    def numeric(entry, single):
        text = float(entry['exported_text'])
        if not math.isfinite(text):
            return False
        assert 'numeric_value' in entry, 'Missing native floating-point number'
        number = entry['numeric_value']
        assert type(number) in [int, float] and math.isfinite(number)
        if single:
            assert struct.unpack('f', struct.pack('f', number))[0] == number
        assert abs(number - text) <= max(5.1e-7, abs(number) * 1e-6)
        return True

    def inspect(value):
        if isinstance(value, dict):
            if value.get('property_class') == 'FloatProperty':
                counts['fields'] += numeric(value, True)
            if value.get('cpp_type') in ['TArray<float>', 'TArray<double>'] and 'elements' in value:
                for entry in value['elements']:
                    counts['array_elements'] += numeric(entry, value['cpp_type'] == 'TArray<float>')
            for child in value.values():
                inspect(child)
        elif isinstance(value, list):
            for child in value:
                inspect(child)

    inspect(data['blueprint_cdo_properties'])
    if 'rig_vm' in data:
        inspect(data['rig_vm']['literal'])
        inspect(data['rig_vm']['work'])
    for case in data.get('rig_node_history_probe', {}).get('cases', []):
        for frame in case['frames']:
            inspect(frame['work_after'])
    assert sum(counts.values()) > 0
    return counts


def self_test():
    single = struct.unpack('f', struct.pack('f', 1.23456789))[0]
    data = {'blueprint_cdo_properties': [{'property_class': 'FloatProperty', 'exported_text': '1.234568', 'numeric_value': single}], 'rig_vm': {'literal': {'properties': [{'cpp_type': 'TArray<float>', 'elements': [{'exported_text': '1.234568', 'numeric_value': single}]}]}, 'work': {'properties': [{'cpp_type': 'TArray<double>', 'elements': [{'exported_text': '1.234568', 'numeric_value': 1.23456789123456}]}]}}}
    assert validate(data) == {'fields': 1, 'array_elements': 2}
    bad_cases = []
    for replacement in [{}, {'numeric_value': True}, {'numeric_value': 1.23456789}]:
        bad = copy.deepcopy(data)
        bad['rig_vm']['literal']['properties'][0]['elements'][0] = {'exported_text': '1.234568', **replacement}
        bad_cases.append(bad)
    nonfinite = {'property_class': 'FloatProperty', 'exported_text': 'nan'}
    bad_cases.append({'blueprint_cdo_properties': [nonfinite]})
    for bad in bad_cases:
        try:
            validate(bad)
        except AssertionError:
            continue
        raise AssertionError('Invalid native array element was accepted')
    assert validate({'blueprint_cdo_properties': data['blueprint_cdo_properties']}) == {'fields': 1, 'array_elements': 0}
    data['blueprint_cdo_properties'].append(nonfinite)
    data['rig_vm']['literal']['properties'][0]['elements'].append({'exported_text': 'inf'})
    assert validate(data) == {'fields': 1, 'array_elements': 2}
    return {'valid_cases': 3, 'rejected_cases': 4}


if __name__ == '__main__':
    result = self_test() if sys.argv[1:] == ['--self-test'] else validate(json.loads(Path(sys.argv[1]).read_text(encoding='utf-8-sig')))
    print(json.dumps(result))
