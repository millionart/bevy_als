import argparse
import copy
import json
from pathlib import Path

from import_camera_component import record
from ue_animation_asset_resolver import source_object_path
from ue_exported_text import parse_list, parse_quoted
from camera_curve_conversion import CAMERA_CURVE_TARGETS


def camera_display_mappings():
    return {target: [source, 1.0 / scale] for source, (target, scale) in sorted(CAMERA_CURVE_TARGETS.items())}


def convert(source):
    if source['asset_class'] != '/Script/Engine.Skeleton':
        raise ValueError('Expected a native skeleton export')
    attached = {source_object_path(parse_quoted(value)) for value in
                parse_list(record(source['asset_properties'], 'AssetUserData')['exported_text'])}
    metadata = [value for value in source['asset_objects']
                if value['class_path'] == '/Script/Engine.AnimCurveMetaData' and value['object_path'] in attached]
    if len(metadata) != 1:
        raise ValueError('Skeleton curve metadata is missing or ambiguous')
    names = [value['key'] for value in record(metadata[0]['properties'], 'CurveMetaData')['elements']]
    if any(not isinstance(name, str) or not name for name in names) or len(set(names)) != len(names):
        raise ValueError('Skeleton curve names are invalid or duplicated')
    return {'source_skeleton': source['asset_path'], 'curves': sorted(names)}


def self_test():
    mappings = camera_display_mappings()
    assert len(mappings) == len(CAMERA_CURVE_TARGETS)
    for source, (target, scale) in CAMERA_CURVE_TARGETS.items():
        name, inverse = mappings[target]
        assert name == source and abs(37.5 * scale * inverse - 37.5) < 1e-9
    assert mappings['CameraOffsetZ'] == ['CameraOffsetX', -100.0]
    assert mappings['LocationLagX'] == ['LocationLagY', 1.0]
    path = '/Game/Fixture.Fixture:Metadata'
    data = {'asset_class': '/Script/Engine.Skeleton', 'asset_path': '/Game/Fixture.Fixture',
            'asset_properties': [{'name': 'AssetUserData', 'exported_text': f'("/Script/Engine.AnimCurveMetaData\'{path}\'")'}],
            'asset_objects': [{'class_path': '/Script/Engine.AnimCurveMetaData', 'object_path': path,
                              'properties': [{'name': 'CurveMetaData', 'elements': [{'key': 'Zulu'}, {'key': 'Alpha'}]}]}]}
    decoy = copy.deepcopy(data['asset_objects'][0])
    decoy['object_path'] = '/Game/Other.Other:Metadata'
    data['asset_objects'].insert(0, decoy)
    assert convert(data)['curves'] == ['Alpha', 'Zulu']
    for mutation in ('missing', 'duplicate', 'class'):
        bad = copy.deepcopy(data)
        if mutation == 'missing':
            bad['asset_objects'].pop()
        elif mutation == 'duplicate':
            bad['asset_objects'][1]['properties'][0]['elements'][0]['key'] = 'Alpha'
        else:
            bad['asset_class'] = '/Script/Engine.AnimSequence'
        try:
            convert(bad)
        except ValueError:
            continue
        raise AssertionError(f'Invalid {mutation} metadata was accepted')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path)
    parser.add_argument('--camera-display', action='store_true')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test()
    if args.source or args.camera_display:
        result = camera_display_mappings() if args.camera_display else convert(json.loads(args.source.read_text(encoding='utf-8-sig')))
        output = args.output or Path('assets/ui/als_camera_curves.runtime.json' if args.camera_display else 'assets/ui/als_character_curves.runtime.json')
        text = json.dumps(result, indent=2) + '\n'
        if args.check:
            if output.read_text(encoding='utf-8') != text:
                raise SystemExit('Debug curve metadata differs from its source contract')
        else:
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(text, encoding='utf-8')
        print(json.dumps({'pass': True, 'entries': len(result) if args.camera_display else len(result['curves'])}))


if __name__ == '__main__':
    main()
