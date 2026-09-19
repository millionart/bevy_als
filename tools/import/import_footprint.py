import argparse
import json
import math
import shutil
from copy import deepcopy
from pathlib import Path

from PIL import Image

from generate_als_notify_manifest import manifest
from coordinate_basis import source_direction_to_runtime, source_point_to_runtime, source_rotation_to_runtime, source_rotator_to_runtime_rotation
from import_camera_component import record, vector
from ue_animation_asset_resolver import asset_property, source_object_path
from ue_exported_text import parse_bool
from audit_material_texture_channels import audit as audit_channels, self_test as channel_self_test


def source_contract(directory):
    sources = {}
    for path in directory.glob('*.ue.json'):
        value = json.loads(path.read_text(encoding='utf-8-sig'))
        if value.get('asset_class', '').endswith('.AlsFootstepEffectsSettings') and not record(value['asset_properties'], 'Effects').get('elements'):
            continue
        if value.get('asset_class', '').endswith('.Texture2D') and not value.get('source_image_pixel_exact'):
            continue
        sources[value['asset_path'].rsplit('.', 1)[-1]] = value
    settings = sources['FES_Als_Default']
    effects = record(settings['asset_properties'], 'Effects')['elements']
    assert len(effects) == 1 and effects[0]['key'] == 'SurfaceType_Default'
    decal = record(effects[0]['fields'], 'Decal')['fields']
    assert record(decal, 'Duration')['exported_text'] == '4.000000'
    assert record(decal, 'FadeOutDuration')['exported_text'] == '2.000000'
    assert record(decal, 'SpawnMode')['exported_text'] == 'SpawnAttachedToTraceHitComponent'
    defaults = sources['AlsAnimNotify_FootstepEffects']['class_default_properties']
    assert record(defaults, 'bSpawnDecal')['exported_text'] == 'True'
    assert record(defaults, 'bSkipEffectsWhenInAir')['exported_text'] == 'False'
    texture = sources['T_Als_Footprint_M']
    assert texture['source_channels'] == 4 and texture['source_pixel_format'] == 'BGRA8'
    assert texture['source_image'].endswith('.tga')
    header = (directory / texture['source_image']).read_bytes()[:18]
    assert header[16] == 32 and header[17] & 15 == 8
    image = Image.open(directory / texture['source_image'])
    assert image.mode == 'RGBA' and image.size == (texture['source_width'], texture['source_height'])
    assert image.size == (512, 512) and image.getextrema()[3] == (0, 255)
    assert asset_property(texture, 'SRGB') == 'False'
    material = sources['M_Als_Footprint_Decal']
    assert asset_property(material, 'MaterialDomain') == 'MD_DeferredDecal'
    assert asset_property(material, 'BlendMode') == 'BLEND_Translucent'
    assert asset_property(sources['BS_Als_WalkRun_Forward'], 'NotifyTriggerMode') == 'HighestWeightedAnimation'
    audit = json.loads((directory / 'notifies.json').read_text())
    assert audit['summary']['failure_count'] == 0
    assert audit['summary']['event_classes']['AlsAnimNotify_FootstepEffects'] == 154
    generated = manifest(audit, Path('assets'))
    assert generated == json.loads(Path('assets/animation_events/als_notify_manifest.json').read_text())
    references = {event['parameters'].get('FootstepEffectsSettings') for clip in generated['clips'] for event in clip['events'] if event['class'] == 'AlsAnimNotify_FootstepEffects'}
    assert references == {'None/' + settings['asset_path'].replace('.', '/')}
    return sources


def compile_definition(sources):
    settings = sources['FES_Als_Default']
    properties = settings['asset_properties']
    defaults = sources['AlsAnimNotify_FootstepEffects']['class_default_properties']
    effects = record(properties, 'Effects')['elements']
    result = {
        'source_settings': settings['asset_path'],
        'notify_settings_reference': 'None/' + settings['asset_path'].replace('.', '/'),
        'trace_channel': asset_property(settings, 'SurfaceTraceChannel'),
        'trace_distance': float(asset_property(settings, 'SurfaceTraceDistance')) * 0.01,
        'spawn_angle_cos': float(asset_property(settings, 'DecalSpawnAngleThresholdCos')),
        'foot_y_axes': [source_direction_to_runtime(vector(asset_property(settings, 'Foot' + side + 'YAxis'))) for side in ('Left', 'Right')],
        'foot_up_axes': [source_direction_to_runtime(vector(asset_property(settings, 'Foot' + side + 'ZAxis'))) for side in ('Left', 'Right')],
        'notify_defaults': {p['name']: (parse_bool(p['exported_text']) if p['property_class'] == 'BoolProperty' else float(p['exported_text']) if p['property_class'] == 'FloatProperty' else p['exported_text']) for p in defaults if p['name'] not in ('NotifyColor', 'bShouldFireInEditor')},
        'effects': [],
    }
    actor = sources['B_Als_Character']
    mesh = next(obj for obj in actor['blueprint_cdo_subobjects'] if obj['object_path'].endswith(':CharacterMesh0'))
    capsule = next(obj for obj in actor['blueprint_cdo_subobjects'] if obj['object_path'].endswith(':CollisionCylinder'))
    result['character_receives_decals'] = parse_bool(record(mesh['properties'], 'bReceivesDecals')['exported_text'])
    assert 'Visibility' not in mesh['blocking_query_channels'] and 'Visibility' not in capsule['blocking_query_channels']
    for effect in effects:
        decal = record(effect['fields'], 'Decal')['fields']
        read = lambda name: record(decal, name)['exported_text']
        rotations = [source_rotation_to_runtime(vector(read('Foot' + side + 'RotationOffsetQuaternion'), 'XYZW')) for side in ('Left', 'Right')]
        for side, rotation in zip(('Left', 'Right'), rotations):
            calculated = source_rotator_to_runtime_rotation(vector(read('Foot' + side + 'RotationOffset'), ('Pitch', 'Yaw', 'Roll')))
            if abs(abs(sum(a * b for a, b in zip(rotation, calculated))) - 1.0) > 1e-5:
                raise ValueError('Source cached decal rotation differs from its authored rotator')
        result['effects'].append({
            'surface': effect['key'],
            'material': read('DecalMaterial'),
            'attached': read('SpawnMode') == 'SpawnAttachedToTraceHitComponent',
            'size': source_point_to_runtime(vector(read('Size'))),
            'location_offset': source_point_to_runtime(vector(read('LocationOffset'))),
            'rotation_offsets': rotations,
            'duration': float(read('Duration')),
            'fade_out_duration': float(read('FadeOutDuration')),
            'sound': record(record(effect['fields'], 'Sound')['fields'], 'Sound')['exported_text'],
            'particle_system': record(record(effect['fields'], 'ParticleSystem')['fields'], 'ParticleSystem')['exported_text'],
        })
    material = sources['M_Als_Footprint_Decal']
    objects = {o['object_path']: o for o in material['asset_objects']}
    editor = next(o for o in objects.values() if o['class_path'].endswith('.MaterialEditorOnlyData'))

    def evaluate(fields, channels, lifetime, fallback=0.0, seen=()):
        reference = record(fields, 'Expression')['exported_text']
        if reference == 'None':
            return fallback
        path = source_object_path(reference)
        if path in seen:
            raise ValueError('Cyclic footprint material expression')
        obj = objects[path]
        props = obj['properties']
        read = lambda name: record(props, name)['exported_text']
        kind = obj['class_path'].rsplit('.', 1)[-1]
        if kind == 'MaterialExpressionConstant':
            return float(read('R'))
        if kind == 'MaterialExpressionScalarParameter':
            return float(read('DefaultValue'))
        if kind == 'MaterialExpressionDecalLifetimeOpacity':
            return lifetime
        if kind == 'MaterialExpressionTextureSample':
            if source_object_path(read('Texture')) != sources['T_Als_Footprint_M']['asset_path'] or read('ConstCoordinate') != '0':
                raise ValueError('Unsupported footprint texture binding')
            index = int(record(fields, 'OutputIndex')['exported_text'])
            if index not in (1, 2, 3, 4):
                raise ValueError('Footprint texture output must be scalar')
            return channels[index - 1]
        if kind == 'MaterialExpressionMultiply':
            return math.prod(evaluate(record(props, name)['fields'], channels, lifetime, float(read('Const' + name)), (*seen, path)) for name in ('A', 'B'))
        raise ValueError('Unsupported footprint material expression: ' + kind)

    scalar = lambda name, rgba=(1, 1, 1, 1), lifetime=1: evaluate(record(editor['properties'], name)['fields'], rgba, lifetime)
    material_definition = {'base_color': scalar('BaseColor'), 'opacity': scalar('Opacity'), 'roughness_scale': scalar('Roughness')}
    for field in ('Metallic', 'Specular'):
        fields = record(editor['properties'], field)['fields']
        if not parse_bool(record(fields, 'UseConstant')['exported_text']):
            raise ValueError('Unsupported footprint material input: ' + field)
        material_definition[field.lower()] = float(record(fields, 'Constant')['exported_text'])
    for rgba in ((0, 0, 0, 0), (0.1, 0.2, 0.3, 0.4), (1, 0, 1, 0), (0, 1, 0, 1)):
        for life in (0, 0.4, 1):
            assert scalar('BaseColor', rgba, life) == material_definition['base_color']
            assert abs(scalar('Opacity', rgba, life) - rgba[3] * life * material_definition['opacity']) < 1e-7
            assert abs(scalar('Roughness', rgba, life) - rgba[1] * material_definition['roughness_scale']) < 1e-7
    result['material'] = {**material_definition, 'texture': 'textures/footprint/T_Als_Footprint_M.tga'}
    return result


def self_test(sources):
    result = compile_definition(sources)
    assert result['trace_distance'] == 0.5
    assert result['foot_up_axes'] == [[1, 0, 0], [-1, 0, 0]]
    assert result['effects'][0]['location_offset'] == [0, -0.0175, -0.1]
    assert result['effects'][0]['size'] == [0.1, 0.2, 0.2]
    assert result['effects'][0]['rotation_offsets'] == [[-0.5, -0.5, 0.5, 0.5], [0.5, -0.5, -0.5, 0.5]]
    assert result['material']['opacity'] == 0.3 and result['material']['roughness_scale'] == 0.2
    changed = deepcopy(sources)
    record(changed['FES_Als_Default']['asset_properties'], 'SurfaceTraceDistance')['exported_text'] = '75.000000'
    assert compile_definition(changed)['trace_distance'] == 0.75
    expression = next(o for o in changed['M_Als_Footprint_Decal']['asset_objects'] if o['class_path'].endswith('.MaterialExpressionScalarParameter'))
    record(expression['properties'], 'DefaultValue')['exported_text'] = '0.600000'
    assert compile_definition(changed)['material']['opacity'] == 0.6


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source-directory', required=True, type=Path)
    parser.add_argument('--asset-root', type=Path, default=Path('assets'))
    parser.add_argument('--write', action='store_true')
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    sources = source_contract(args.source_directory)
    if args.self_test:
        self_test(sources)
    if args.write or args.check:
        definition = compile_definition(sources)
        output = args.asset_root / 'effects/footprint.runtime.json'
        text = json.dumps(definition, indent=2) + '\n'
        if args.write:
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(text, encoding='utf-8')
        else:
            assert output.read_text(encoding='utf-8') == text
        raw = args.source_directory / sources['T_Als_Footprint_M']['source_image']
        saved = args.asset_root / definition['material']['texture']
        if args.write and not saved.exists():
            saved.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(raw, saved)
        assert saved.read_bytes() == raw.read_bytes()
        image = Image.open(saved)
        shader = (args.asset_root / 'shaders/packed_decal.wgsl').read_text(encoding='utf-8')
        audit_channels(sources['M_Als_Footprint_Decal'], sources['T_Als_Footprint_M'], image, Image.open(raw), shader, 'packed_texture')
        if args.self_test:
            channel_self_test(sources['M_Als_Footprint_Decal'], sources['T_Als_Footprint_M'], image, shader, 'packed_texture')
    print(json.dumps({'source_assets': len(sources), 'footstep_notifies': 154, 'native_map_defaults': 'PASS', 'native_class_defaults': 'PASS', 'source_texture': 'PASS', 'existing_notify_manifest': 'PASS'}))


if __name__ == '__main__':
    main()
