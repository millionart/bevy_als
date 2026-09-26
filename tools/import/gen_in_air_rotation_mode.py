import argparse
import json
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/animgraph_authoring'))
from ue_animation_asset_resolver import asset_property
from ue_animgraph_converter import content_root_for_uasset, read_uasset_source


def source_mode(character, settings):
    if character.get('asset_class') != '/Script/Engine.Blueprint' or settings.get('asset_class') != '/Script/ALS.AlsCharacterSettings':
        raise ValueError('source character or settings class is invalid')
    reference = next((value['exported_text'] for value in character.get('blueprint_cdo_properties', []) if value['name'] == 'Settings'), '')
    if settings['asset_path'] not in reference:
        raise ValueError('character does not use the selected settings asset')
    value = next((value for value in settings['asset_properties'] if value['name'] == 'InAirRotationMode'), None)
    if value is None:
        raise ValueError('source in-air rotation mode is missing')
    mode = int(value['enum_underlying_value'])
    if (mode, value.get('enum_value_name')) not in ((0, 'KeepWorldRotation'), (3, 'EAlsInAirRotationMode_MAX')):
        raise ValueError('source in-air rotation mode is unsupported')
    return mode


def self_test():
    settings = {'asset_path': '/Test/CS.CS', 'asset_class': '/Script/ALS.AlsCharacterSettings',
                'asset_properties': [{'name': 'InAirRotationMode', 'enum_underlying_value': '3', 'enum_value_name': 'EAlsInAirRotationMode_MAX'}]}
    character = {'asset_class': '/Script/Engine.Blueprint',
                 'blueprint_cdo_properties': [{'name': 'Settings', 'exported_text': "/Script/ALS.AlsCharacterSettings'/Test/CS.CS'"}]}
    assert source_mode(character, settings) == 3
    settings['asset_properties'][0]['enum_underlying_value'] = '2'
    settings['asset_properties'][0]['enum_value_name'] = 'RotateToVelocity'
    try:
        source_mode(character, settings)
    except ValueError:
        pass
    else:
        raise AssertionError('unsupported source mode was accepted')
    settings['asset_properties'][0]['enum_underlying_value'] = '0'
    settings['asset_properties'][0]['enum_value_name'] = 'KeepWorldRotation'
    assert source_mode(character, settings) == 0
    character['blueprint_cdo_properties'][0]['exported_text'] = 'None'
    try:
        source_mode(character, settings)
    except ValueError:
        pass
    else:
        raise AssertionError('unreferenced settings asset was accepted')
    print('In-air rotation mode conversion self-test PASS', flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--character-uasset', type=Path)
    parser.add_argument('--settings-uasset', type=Path)
    parser.add_argument('--source-reader', type=Path, default=ROOT / 'tools/run_ue_source_reader.ps1')
    parser.add_argument('--output', type=Path, default=ROOT / 'src/als/settings/generated/in_air_rotation_mode.rs')
    parser.add_argument('--report', type=Path, default=ROOT / 'target/in_air_rotation_mode.report.json')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    report = {'success': False}
    try:
        if args.character_uasset is None or args.settings_uasset is None:
            raise ValueError('character and settings uassets are required')
        character, _ = read_uasset_source(args.character_uasset, content_root_for_uasset(args.character_uasset, None), '/ALS', args.source_reader, 300, 150)
        settings, _ = read_uasset_source(args.settings_uasset, content_root_for_uasset(args.settings_uasset, None), '/ALS', args.source_reader, 300, 150)
        mode = source_mode(character, settings)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(f'{mode}u8\n', encoding='utf-8')
        report.update(success=True, character=character['asset_path'], settings=settings['asset_path'], source_mode=mode)
    except (ValueError, OSError, KeyError, RuntimeError) as error:
        report['error'] = str(error)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report), flush=True)
    if not report['success']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
