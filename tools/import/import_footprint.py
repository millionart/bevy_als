import argparse
import json
import math
import shutil
from copy import deepcopy
from pathlib import Path

from PIL import Image

from generate_als_notify_manifest import manifest
from coordinate_basis import quaternion_multiply, source_direction_to_runtime, source_point_to_runtime, source_rotation_to_runtime, source_rotator_to_runtime_rotation
from import_camera_component import record, vector
from ue_animation_asset_resolver import asset_property, source_object_path
from ue_exported_text import parse_bool
from audit_material_texture_channels import audit as audit_channels, self_test as channel_self_test


def named(sources, name):
    matches = [value for value in sources.values() if value['asset_path'].rsplit('.', 1)[-1] == name]
    if len(matches) != 1:
        raise ValueError('Source asset name is missing or ambiguous: ' + name)
    return matches[0]


def source_contract(directory):
    sources = {}
    for path in directory.glob('*.ue.json'):
        value = json.loads(path.read_text(encoding='utf-8-sig'))
        if value.get('asset_class', '').endswith('.AlsFootstepEffectsSettings') and 'elements' not in record(value['asset_properties'], 'Effects'):
            continue
        if value.get('asset_class', '').endswith('.Texture2D') and not value.get('source_image_pixel_exact'):
            continue
        sources[value['asset_path']] = value
    defaults = named(sources, 'AlsAnimNotify_FootstepEffects')['class_default_properties']
    assert record(defaults, 'bSpawnDecal')['exported_text'] == 'True'
    assert record(defaults, 'bSkipEffectsWhenInAir')['exported_text'] == 'False'
    texture = named(sources, 'T_Als_Footprint_M')
    assert texture['source_channels'] == 4 and texture['source_pixel_format'] == 'BGRA8'
    assert texture['source_image'].endswith('.tga')
    header = (directory / texture['source_image']).read_bytes()[:18]
    assert header[16] == 32 and header[17] & 15 == 8
    image = Image.open(directory / texture['source_image'])
    assert image.mode == 'RGBA' and image.size == (texture['source_width'], texture['source_height'])
    assert image.size == (512, 512) and image.getextrema()[3] == (0, 255)
    assert asset_property(texture, 'SRGB') == 'False'
    assert asset_property(named(sources, 'BS_Als_WalkRun_Forward'), 'NotifyTriggerMode') == 'HighestWeightedAnimation'
    audit = json.loads((directory / 'notifies.json').read_text())
    assert audit['summary']['failure_count'] == 0
    assert audit['summary']['event_classes']['AlsAnimNotify_FootstepEffects'] == 154
    generated = manifest(audit, Path('assets'))
    assert generated == json.loads(Path('assets/ALS/Animations/als_notify_manifest.json').read_text())
    references = {event['parameters'].get('FootstepEffectsSettings') for clip in generated['clips'] for event in clip['events'] if event['class'] == 'AlsAnimNotify_FootstepEffects'}
    assert references <= {'None/' + value['asset_path'].replace('.', '/') for value in sources.values()
                          if value.get('asset_class', '').endswith('.AlsFootstepEffectsSettings')}
    return sources


def compile_definition(sources, settings):
    properties = settings['asset_properties']
    if asset_property(settings, 'SurfaceTraceChannel') != 'ECC_Visibility':
        raise ValueError('Unsupported footprint trace layer')
    defaults = named(sources, 'AlsAnimNotify_FootstepEffects')['class_default_properties']
    effects = record(properties, 'Effects')['elements']
    result = {
        'source_settings': settings['asset_path'],
        'notify_settings_reference': 'None/' + settings['asset_path'].replace('.', '/'),
        'trace_layer': 'Visibility',
        'trace_distance': float(asset_property(settings, 'SurfaceTraceDistance')) * 0.01,
        'spawn_angle_cos': float(asset_property(settings, 'DecalSpawnAngleThresholdCos')),
        'foot_y_axes': [source_direction_to_runtime(vector(asset_property(settings, 'Foot' + side + 'YAxis'))) for side in ('Left', 'Right')],
        'foot_up_axes': [source_direction_to_runtime(vector(asset_property(settings, 'Foot' + side + 'ZAxis'))) for side in ('Left', 'Right')],
        'notify_defaults': {p['name']: (parse_bool(p['exported_text']) if p['property_class'] == 'BoolProperty' else float(p['exported_text']) if p['property_class'] == 'FloatProperty' else p['exported_text']) for p in defaults if p['name'] not in ('NotifyColor', 'bShouldFireInEditor')},
        'effects': [],
        'materials': {},
    }
    for effect in effects:
        decal = record(effect['fields'], 'Decal')['fields']
        read = lambda name: record(decal, name)['exported_text']
        rotations = [source_rotation_to_runtime(vector(read('Foot' + side + 'RotationOffsetQuaternion'), 'XYZW')) for side in ('Left', 'Right')]
        for side, rotation in zip(('Left', 'Right'), rotations):
            calculated = source_rotator_to_runtime_rotation(vector(read('Foot' + side + 'RotationOffset'), ('Pitch', 'Yaw', 'Roll')))
            if abs(abs(sum(a * b for a, b in zip(rotation, calculated))) - 1.0) > 1e-5:
                raise ValueError('Source cached decal rotation differs from its authored rotator')
        size = source_point_to_runtime(vector(read('Size')))
        offset = source_point_to_runtime(vector(read('LocationOffset')))
        result['effects'].append({
            'surface': effect['key'],
            'material': None if read('DecalMaterial') == 'None' else read('DecalMaterial'),
            'attached': read('SpawnMode') == 'SpawnAttachedToTraceHitComponent',
            'size': [size[1], -size[2], size[0]],
            'location_offset': [offset[1], offset[2], offset[0]],
            'rotation_offsets': [quaternion_multiply(rotation, [0.5, 0.5, 0.5, 0.5]) for rotation in rotations],
            'duration': float(read('Duration')),
            'fade_out_duration': float(read('FadeOutDuration')),
            'sound': record(record(effect['fields'], 'Sound')['fields'], 'Sound')['exported_text'],
            'particle_system': record(record(effect['fields'], 'ParticleSystem')['fields'], 'ParticleSystem')['exported_text'],
        })
        material = result['effects'][-1]['material']
        if material is not None and material not in result['materials']:
            result['materials'][material] = compile_material(sources, sources[material])
    return result


def compile_material(sources, material):
    if asset_property(material, 'MaterialDomain') != 'MD_DeferredDecal' or asset_property(material, 'BlendMode') != 'BLEND_Translucent':
        raise ValueError('Unsupported footprint decal material domain or blend mode')
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
            if source_object_path(read('Texture')) != named(sources, 'T_Als_Footprint_M')['asset_path'] or read('ConstCoordinate') != '0':
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
    return {**material_definition, 'texture': 'ALS/MaterialLibrary/Footprint/T_Als_Footprint_M.tga'}


def compile_library(sources):
    actor = named(sources, 'B_Als_Character')
    mesh = next(obj for obj in actor['blueprint_cdo_subobjects'] if obj['object_path'].endswith(':CharacterMesh0'))
    capsule = next(obj for obj in actor['blueprint_cdo_subobjects'] if obj['object_path'].endswith(':CollisionCylinder'))
    assert 'Visibility' not in mesh['blocking_query_channels'] and 'Visibility' not in capsule['blocking_query_channels']
    return {'character_receives_decals': parse_bool(record(mesh['properties'], 'bReceivesDecals')['exported_text']),
            'definitions': [compile_definition(sources, value) for _, value in sorted(sources.items())
                            if value.get('asset_class', '').endswith('.AlsFootstepEffectsSettings')]}


def self_test(sources):
    settings = named(sources, 'FES_Als_Default')
    result = compile_definition(sources, settings)
    assert result['trace_distance'] == 0.5
    assert result['trace_layer'] == 'Visibility'
    assert result['foot_up_axes'] == [[1, 0, 0], [-1, 0, 0]]
    effect = result['effects'][0]
    assert effect['location_offset'] == [-0.0175, -0.1, 0]
    assert effect['size'] == [0.2, -0.2, 0.1]
    assert effect['rotation_offsets'] == [[-0.5, 0.5, 0.5, 0.5], [0.5, -0.5, 0.5, 0.5]]
    rotate = lambda q, point: quaternion_multiply(quaternion_multiply(q, [*point, 0.0]), [-q[0], -q[1], -q[2], q[3]])[:3]
    for old_rotation, rotation in zip(([-0.5, -0.5, 0.5, 0.5], [0.5, -0.5, -0.5, 0.5]), effect['rotation_offsets']):
        assert all(abs(a - b) < 1e-8 for a, b in zip(rotate(old_rotation, [0, -0.0175, -0.1]), rotate(rotation, effect['location_offset'])))
        for point in ([0.2, -0.4, 0.1], [-0.3, 0.1, 0.4], [0.5, 0.5, -0.5]):
            projected = [point[1], -point[2], point[0]]
            old_world = rotate(old_rotation, [value * size for value, size in zip(point, [0.1, 0.2, 0.2])])
            new_world = rotate(rotation, [value * size for value, size in zip(projected, effect['size'])])
            assert all(abs(a - b) < 1e-8 for a, b in zip(old_world, new_world))
            assert [projected[0] + 0.5, -projected[1] + 0.5] == [point[1] + 0.5, point[2] + 0.5]
            assert abs(projected[2]) == abs(point[0])
    material_key = result['effects'][0]['material']
    assert result['materials'][material_key]['opacity'] == 0.3 and result['materials'][material_key]['roughness_scale'] == 0.2
    changed = deepcopy(sources)
    changed_settings = changed[settings['asset_path']]
    record(changed_settings['asset_properties'], 'SurfaceTraceDistance')['exported_text'] = '75.000000'
    assert compile_definition(changed, changed_settings)['trace_distance'] == 0.75
    expression = next(o for o in changed[material_key]['asset_objects'] if o['class_path'].endswith('.MaterialExpressionScalarParameter'))
    record(expression['properties'], 'DefaultValue')['exported_text'] = '0.600000'
    assert compile_definition(changed, changed_settings)['materials'][material_key]['opacity'] == 0.6
    other_material = deepcopy(changed[material_key])
    other_material['asset_path'] = '/ALS/Tests/M_Alternate.M_Alternate'
    other_parameter = next(o for o in other_material['asset_objects'] if o['class_path'].endswith('.MaterialExpressionScalarParameter'))
    record(other_parameter['properties'], 'DefaultValue')['exported_text'] = '0.800000'
    changed[other_material['asset_path']] = other_material
    alternate = deepcopy(changed_settings)
    alternate['asset_path'] = '/ALS/Tests/Footstep/Alternate.Alternate'
    effects = record(alternate['asset_properties'], 'Effects')['elements']
    effects.append(deepcopy(effects[0]))
    effects[1]['key'] = 'SurfaceType1'
    decal = record(effects[1]['fields'], 'Decal')['fields']
    record(decal, 'Duration')['exported_text'] = '9.000000'
    record(decal, 'DecalMaterial')['exported_text'] = other_material['asset_path']
    changed[alternate['asset_path']] = alternate
    library = compile_library(changed)
    compiled = next(value for value in library['definitions'] if value['source_settings'] == alternate['asset_path'])
    assert compiled['effects'][1]['duration'] == 9
    assert compiled['effects'][1]['material'] == other_material['asset_path']
    assert compiled['materials'][other_material['asset_path']]['opacity'] == 0.8
    assert compiled['notify_settings_reference'] != result['notify_settings_reference']
    for field, value in (('MaterialDomain', 'MD_Surface'), ('BlendMode', 'BLEND_Opaque')):
        invalid = deepcopy(other_material)
        record(invalid['asset_properties'], field)['exported_text'] = value
        try:
            compile_material(changed, invalid)
        except ValueError as error:
            assert str(error) == 'Unsupported footprint decal material domain or blend mode'
        else:
            raise AssertionError('Unsupported decal material was accepted')
    record(decal, 'DecalMaterial')['exported_text'] = 'None'
    assert compile_definition(changed, alternate)['effects'][1]['material'] is None
    effects.clear()
    assert compile_definition(changed, alternate)['effects'] == []
    record(changed_settings['asset_properties'], 'SurfaceTraceChannel')['exported_text'] = 'ECC_Camera'
    try:
        compile_definition(changed, changed_settings)
    except ValueError as error:
        assert str(error) == 'Unsupported footprint trace layer'
    else:
        raise AssertionError('Unsupported source trace layer was accepted')


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
        definition = compile_library(sources)
        output = args.asset_root / 'ALS/Data/Footstep/footprint.runtime.json'
        text = json.dumps(definition, indent=2) + '\n'
        if args.write:
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(text, encoding='utf-8')
        else:
            assert output.read_text(encoding='utf-8') == text
        texture = named(sources, 'T_Als_Footprint_M')
        raw = args.source_directory / texture['source_image']
        saved = args.asset_root / 'ALS/MaterialLibrary/Footprint/T_Als_Footprint_M.tga'
        if args.write and not saved.exists():
            saved.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(raw, saved)
        assert saved.read_bytes() == raw.read_bytes()
        image = Image.open(saved)
        shader = (args.asset_root / 'Bevy/Materials/packed_decal.wgsl').read_text(encoding='utf-8')
        audit_channels(named(sources, 'M_Als_Footprint_Decal'), texture, image, Image.open(raw), shader, 'mesh_view_bindings::clustered_decal_textures')
        if args.self_test:
            channel_self_test(named(sources, 'M_Als_Footprint_Decal'), texture, image, shader, 'mesh_view_bindings::clustered_decal_textures')
    print(json.dumps({'source_assets': len(sources), 'footstep_notifies': 154, 'native_map_defaults': 'PASS', 'native_class_defaults': 'PASS', 'source_texture': 'PASS', 'existing_notify_manifest': 'PASS'}))


if __name__ == '__main__':
    main()
