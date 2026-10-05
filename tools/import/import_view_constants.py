import argparse
import json
import math
import re
from copy import deepcopy
from pathlib import Path


def property_value(properties, name):
    matches = [p['exported_text'] for p in properties if p['name'] == name]
    if len(matches) != 1:
        raise ValueError(f'Source property is missing or ambiguous: {name}')
    return matches[0]


def extract_limit(controller, manager):
    if property_value(controller['blueprint_cdo_properties'], 'PlayerCameraManagerClass') != 'None':
        raise ValueError('Custom player camera manager is unsupported')
    if manager['asset_path'] != '/Script/Engine.PlayerCameraManager':
        raise ValueError('Native player camera manager is required')
    properties = manager['class_default_properties']
    minimum = float(property_value(properties, 'ViewPitchMin'))
    maximum = float(property_value(properties, 'ViewPitchMax'))
    if not math.isfinite(maximum) or not 0 < maximum < 180 or minimum != -maximum:
        raise ValueError('Symmetric source pitch limits below 180 degrees are required')
    return maximum


def generate(text, limit):
    pattern = r'pub const PITCH_LIMIT_DEGREES: f32 = [^;]+;'
    if len(re.findall(pattern, text)) != 1:
        raise ValueError('Target pitch constant is missing or ambiguous')
    return re.sub(pattern, f'pub const PITCH_LIMIT_DEGREES: f32 = {limit!r};', text)


def self_test():
    controller = {'blueprint_cdo_properties': [{'name': 'PlayerCameraManagerClass', 'exported_text': 'None'}]}
    manager = {'asset_path': '/Script/Engine.PlayerCameraManager', 'class_default_properties': [
        {'name': 'ViewPitchMin', 'exported_text': '-89.900002'},
        {'name': 'ViewPitchMax', 'exported_text': '89.900002'}]}
    text = 'pub const PITCH_LIMIT_DEGREES: f32 = 85.0;\n'
    assert generate(text, extract_limit(controller, manager)) == 'pub const PITCH_LIMIT_DEGREES: f32 = 89.900002;\n'
    changed = deepcopy(manager)
    changed['class_default_properties'][0]['exported_text'] = '-70.0'
    changed['class_default_properties'][1]['exported_text'] = '70.0'
    assert generate(text, extract_limit(controller, changed)) == 'pub const PITCH_LIMIT_DEGREES: f32 = 70.0;\n'
    for operation in ['missing', 'duplicate', 'asymmetric', 'nan']:
        broken = deepcopy(manager)
        properties = broken['class_default_properties']
        if operation == 'missing':
            properties.pop()
        elif operation == 'duplicate':
            properties.append(deepcopy(properties[1]))
        else:
            properties[1]['exported_text'] = '80' if operation == 'asymmetric' else 'nan'
        try:
            extract_limit(controller, broken)
        except ValueError:
            pass
        else:
            raise AssertionError(f'Invalid source accepted: {operation}')
    for broken in ['', text + text]:
        try:
            generate(broken, 89.9)
        except ValueError:
            pass
        else:
            raise AssertionError('Invalid target accepted')
    print('View constants self-test PASS')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--controller', type=Path)
    parser.add_argument('--manager', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    if not all([args.controller, args.manager, args.output]):
        parser.error('--controller, --manager and --output are required')
    controller = json.loads(args.controller.read_text(encoding='utf-8-sig'))
    manager = json.loads(args.manager.read_text(encoding='utf-8-sig'))
    original = args.output.read_text(encoding='utf-8')
    generated = generate(original, extract_limit(controller, manager))
    if args.check:
        if original != generated:
            raise ValueError('Target pitch constant differs from source')
    else:
        args.output.write_text(generated, encoding='utf-8', newline='\n')
    print('View constants source check PASS' if args.check else 'View constants generated')


if __name__ == '__main__':
    main()
