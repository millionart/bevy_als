import argparse
import json
import math
import sys
from copy import deepcopy
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'animgraph_authoring'))
from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE, source_point_to_runtime, source_direction_to_runtime, source_rotator_to_runtime_rotation
from ue_animation_asset_resolver import asset_property, object_property, source_object_path
from ue_animgraph_materializer import graph_path
from ue_animgraph_pose_contract import generated_anim_blueprint_asset
from ue_exported_text import parse_bool, parse_number, parse_quoted, parse_struct, parse_list


def record(properties, name):
    matches = [value for value in properties if value['name'] == name]
    if len(matches) != 1:
        raise ValueError(f'Property is missing or ambiguous: {name}')
    return matches[0]


def vector(text, axes='XYZ'):
    values = parse_struct(text)
    return [parse_number(values[axis]) for axis in axes]


def wave(properties, name, amplitude_multiplier, frequency_multiplier, scale):
    fields = record(properties, name)['fields']
    values = {value['name']: value['exported_text'] for value in fields}
    mode = values['InitialOffsetType']
    if mode not in {'Random', 'Zero'}:
        raise ValueError('Unsupported wave initial offset')
    return {'amplitude': parse_number(values['Amplitude']) * amplitude_multiplier * scale,
        'frequency_hz': parse_number(values['Frequency']) * frequency_multiplier, 'random_start': mode == 'Random'}


def camera_sockets(mesh, skeleton):
    result = {}
    for asset in [mesh, skeleton]:
        objects = {value['object_path']: value for value in asset['asset_objects']}
        text = asset_property(asset, 'Sockets')
        for reference in parse_list(text) if text else []:
            path = source_object_path(parse_quoted(reference))
            socket = objects[path]
            name = object_property(socket, 'SocketName')
            result.setdefault(name.lower(), {
                'name': name,
                'bone': object_property(socket, 'BoneName'),
                'location': tuple(source_point_to_runtime(vector(object_property(socket, 'RelativeLocation')))),
            })
    return list(result.values())


def compile_definition(actor, mesh, settings, physics, shake, character_mesh, character_skeleton, controller, manager, local_player):
    if object_property({'properties': controller['blueprint_cdo_properties']}, 'PlayerCameraManagerClass') != 'None':
        raise ValueError('Custom player camera manager is unsupported')
    if parse_bool(asset_property(manager, 'bIsOrthographic')) or parse_bool(asset_property(manager, 'bDefaultConstrainAspectRatio')):
        raise ValueError('Camera projection mode is unsupported')
    if asset_property(local_player, 'AspectRatioAxisConstraint') != 'AspectRatio_MaintainYFOV':
        raise ValueError('Camera aspect ratio constraint is unsupported')
    view_aspect_ratio = parse_number(asset_property(manager, 'DefaultAspectRatio'))
    if not math.isfinite(view_aspect_ratio) or view_aspect_ratio <= 0:
        raise ValueError('Camera view aspect ratio must be positive')
    cdo = {'properties': actor['blueprint_cdo_properties']}
    component_path = source_object_path(object_property(cdo, 'Camera'))
    component = next(item for item in actor['blueprint_cdo_subobjects'] if item['object_path'] == component_path)
    props = component['properties']
    owner_mesh = next(item for item in actor['blueprint_cdo_subobjects'] if item['object_path'].endswith(':CharacterMesh0'))
    if source_object_path(object_property(owner_mesh, 'SkeletalMeshAsset')) != character_mesh['asset_path']:
        raise ValueError('Character camera socket mesh reference differs')
    if character_mesh['skeletal_mesh']['skeleton_path'] != character_skeleton['asset_path']:
        raise ValueError('Character camera socket skeleton reference differs')
    if object_property(component, 'AttachParent') != 'CharacterMesh0':
        raise ValueError('Unsupported camera attachment parent')
    if source_object_path(object_property(component, 'Settings')) != settings['asset_path']:
        raise ValueError('Camera settings reference differs')
    if source_object_path(object_property(component, 'SkeletalMeshAsset')) != mesh['asset_path']:
        raise ValueError('Camera mesh reference differs')
    if source_object_path(asset_property(mesh, 'PhysicsAsset')) != physics['asset_path']:
        raise ValueError('Camera physics asset reference differs')
    if object_property(component, 'PhysicsAssetOverride') != 'None':
        raise ValueError('Camera physics override is unsupported')
    if object_property(component, 'VisibilityBasedAnimTickOption') != 'AlwaysTickPoseAndRefreshBones':
        raise ValueError('Camera hidden pose update differs')
    tick = parse_struct(object_property(component, 'PrimaryComponentTick'))
    if tick.get('TickGroup') != 'TG_PostPhysics' or not parse_bool(tick['bCanEverTick']):
        raise ValueError('Camera tick phase differs')
    body_fields = record(props, 'BodyInstance')['fields']
    collision = record(body_fields, 'CollisionEnabled')['exported_text']
    simulate = parse_bool(record(body_fields, 'bSimulatePhysics')['exported_text'])
    if collision != 'NoCollision' or simulate:
        raise ValueError('Camera collision activation is unsupported')
    first = parse_struct(asset_property(settings, 'FirstPerson'))
    third = parse_struct(asset_property(settings, 'ThirdPerson'))
    post_process_weight = parse_number(object_property(component, 'PostProcessWeight'))
    post_process = record(settings['asset_properties'], 'PostProcess')['fields']
    overrides = [value['name'] for value in post_process if value['name'].startswith('bOverride_') and parse_bool(value['exported_text'])]
    blendables = parse_struct(record(post_process, 'WeightedBlendables')['exported_text'])
    if post_process_weight > 1e-8 and (overrides or parse_list(blendables.get('Array', '()'))):
        raise ValueError('Active camera post-process settings are unsupported')
    smoothing = parse_struct(third['TraceDistanceSmoothing'])
    if third['TraceChannel'] != 'ECC_Visibility':
        raise ValueError('Camera trace channel is unsupported')
    physical = physics['physics_asset']
    if len(physical['bodies']) != 1 or physical['constraints'] or physical['collision_disabled_pairs']:
        raise ValueError('Camera physics topology differs')
    body = physical['bodies'][0]
    if len(body['shapes']) != 1 or body['shapes'][0]['kind'] != 'box':
        raise ValueError('Camera physics shape differs')
    box = body['shapes'][0]
    shake_cdo = {'properties': shake['blueprint_cdo_properties']}
    pattern_path = source_object_path(object_property(shake_cdo, 'RootShakePattern'))
    pattern = next(item for item in shake['blueprint_cdo_subobjects'] if item['object_path'] == pattern_path)
    if pattern['class_path'] != '/Script/EngineCameras.WaveOscillatorCameraShakePattern':
        raise ValueError('Camera shake pattern is unsupported')
    location_amplitude = parse_number(object_property(pattern, 'LocationAmplitudeMultiplier'))
    location_frequency = parse_number(object_property(pattern, 'LocationFrequencyMultiplier'))
    rotation_amplitude = parse_number(object_property(pattern, 'RotationAmplitudeMultiplier'))
    rotation_frequency = parse_number(object_property(pattern, 'RotationFrequencyMultiplier'))
    material = parse_struct(parse_list(asset_property(mesh, 'Materials'))[0])['MaterialInterface']
    graph = generated_anim_blueprint_asset(json.dumps(object_property(component, 'AnimClass')))
    return {
        'view_aspect_ratio': view_aspect_ratio,
        'settings': {'ignore_time_dilation': parse_bool(asset_property(settings, 'bIgnoreTimeDilation')),
            'teleport_distance_threshold': parse_number(asset_property(settings, 'TeleportDistanceThreshold')) * SOURCE_LENGTH_TO_RUNTIME_SCALE,
            'trace_radius': parse_number(third['TraceRadius']) * SOURCE_LENGTH_TO_RUNTIME_SCALE,
            'trace_override_offset': tuple(source_point_to_runtime(vector(third['TraceOverrideOffset']))),
            'trace_distance_smoothing': parse_bool(third['bEnableTraceDistanceSmoothing']),
            'trace_distance_smoothing_half_life': parse_number(smoothing['InterpolationHalfLife']),
            'third_person_field_of_view_deg': parse_number(third['FieldOfView']),
            'first_person_field_of_view_deg': parse_number(first['FieldOfView']),
            'field_of_view_min_deg': 5.0, 'field_of_view_max_deg': 175.0},
        'first_person_socket': parse_quoted(first['CameraSocketName']),
        'pivot_sockets': tuple(parse_quoted(third[key]) for key in ['FirstPivotSocketName', 'SecondPivotSocketName']),
        'shoulder_sockets': tuple(parse_quoted(third[key]) for key in ['TraceShoulderLeftSocketName', 'TraceShoulderRightSocketName']),
        'sockets': camera_sockets(character_mesh, character_skeleton),
        'standing_eye_height': parse_number(object_property(cdo, 'BaseEyeHeight')) * SOURCE_LENGTH_TO_RUNTIME_SCALE,
        'crouching_eye_height': parse_number(object_property(cdo, 'CrouchedEyeHeight')) * SOURCE_LENGTH_TO_RUNTIME_SCALE,
        'graph': graph_path(graph, 'AnimGraph', 'generated'),
        'skeleton': 'skeletons/SKM_Als_Camera.static.skn.ron',
        'mesh_scene': 'models/ALSCamera/SKM_Als_Camera.gltf#Scene0',
        'source_material': source_object_path(parse_quoted(material)),
        'attachment_translation': tuple(source_point_to_runtime(vector(object_property(component, 'RelativeLocation')))),
        'attachment_rotation': tuple(source_rotator_to_runtime_rotation(vector(object_property(component, 'RelativeRotation'), ['Pitch', 'Yaw', 'Roll']))),
        'attachment_scale': tuple(source_direction_to_runtime(vector(object_property(component, 'RelativeScale3D')))),
        'auto_activate': parse_bool(object_property(component, 'bAutoActivate')),
        'hidden_in_game': parse_bool(object_property(component, 'bHiddenInGame')),
        'right_shoulder': parse_bool(object_property(component, 'bRightShoulder')),
        'override_field_of_view': parse_bool(object_property(component, 'bOverrideFieldOfView')),
        'field_of_view_override_deg': parse_number(object_property(component, 'FieldOfViewOverride')),
        'post_process_weight': post_process_weight,
        'collision_enabled': False, 'simulate_physics': simulate,
        'physics_body': {'bone': body['bone_name'], 'center': tuple(source_point_to_runtime([box['center'][axis] for axis in 'xyz'])),
            'rotation': tuple(source_rotator_to_runtime_rotation([box['rotation'][axis] for axis in ['pitch', 'yaw', 'roll']])),
            'half_extents': tuple(value * 0.5 for value in source_point_to_runtime([box[axis] for axis in 'xyz'])),
            'mass_kg': body['calculated_mass'], 'default_collision': record(body['default_instance_properties'], 'CollisionEnabled')['exported_text']},
        'sprint_shake': {'single_instance': parse_bool(object_property(shake_cdo, 'bSingleInstance')),
            'duration': parse_number(object_property(pattern, 'Duration')), 'blend_in': parse_number(object_property(pattern, 'BlendInTime')),
            'blend_out': parse_number(object_property(pattern, 'BlendOutTime')),
            'location': tuple(wave(pattern['properties'], name, location_amplitude, location_frequency, scale * SOURCE_LENGTH_TO_RUNTIME_SCALE) for name, scale in [('Y', 1.0), ('Z', 1.0), ('X', -1.0)]),
            'rotation_yxz': tuple(wave(pattern['properties'], name, rotation_amplitude, rotation_frequency, scale) for name, scale in [('Yaw', -math.pi / 180), ('Pitch', math.pi / 180), ('Roll', -math.pi / 180)]),
            'field_of_view': wave(pattern['properties'], 'FOV', 1.0, 1.0, 1.0)},
    }


def literal(value, depth=0):
    if isinstance(value, dict):
        pad = '    ' * (depth + 1)
        return '(\n' + ''.join(pad + key + ': ' + literal(item, depth + 1) + ',\n' for key, item in value.items()) + '    ' * depth + ')'
    if isinstance(value, tuple):
        return '(' + ', '.join(literal(item, depth) for item in value) + ')'
    if isinstance(value, list):
        return '[' + ', '.join(literal(item, depth) for item in value) + ']'
    if isinstance(value, float) and not math.isfinite(value):
        raise ValueError('Camera target value is non-finite')
    return json.dumps(value, ensure_ascii=False)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source-directory', type=Path, required=True)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    def read(name):
        return json.loads((args.source_directory / (name + '.ue.json')).read_text(encoding='utf-8-sig'))
    sources = [read(name) for name in ['B_Als_Character', 'SKM_Als_Camera', 'CS_Als_Default', 'PA_Als_Camera', 'B_Als_CameraShake_Sprint', 'SKM_Als', 'SK_Als', 'B_Als_PlayerController', 'PlayerCameraManager', 'LocalPlayer']]
    result = compile_definition(*sources)
    if args.self_test:
        before = deepcopy(sources)
        assert result['settings']['trace_radius'] == 0.15
        assert result['view_aspect_ratio'] == 1.33333
        changed = deepcopy(sources)
        record(changed[9]['asset_properties'], 'AspectRatioAxisConstraint')['exported_text'] = 'AspectRatio_MaintainXFOV'
        try:
            compile_definition(*changed)
        except ValueError as error:
            assert 'aspect ratio constraint' in str(error)
        else:
            raise AssertionError('Unsupported projection constraint was accepted')
        assert result['physics_body']['half_extents'] == (0.3, 0.15, 0.17500000000000002)
        assert result['physics_body']['center'] == (-0.2, 0.0, 0.0)
        assert result['sprint_shake']['location'][2]['amplitude'] == -0.01
        assert result['sprint_shake']['rotation_yxz'][1]['amplitude'] > 0
        assert result['sprint_shake']['field_of_view']['amplitude'] == 0.0
        sockets = {value['name']: value for value in result['sockets']}
        assert sockets['FirstPersonCamera'] == {'name': 'FirstPersonCamera', 'bone': 'head', 'location': (0.04, 0.0, 0.14)}
        assert sockets['ThirdPersonTraceShoulderRight']['location'] == (0.05, 0.2, 0.0)
        assert sockets['ThirdPersonTraceShoulderLeft']['location'] == (0.05, -0.2, 0.0)
        changed = deepcopy(sources)
        override = deepcopy(next(value for value in changed[6]['asset_objects'] if value['class_path'].endswith('SkeletalMeshSocket')))
        override['object_path'] = changed[5]['asset_path'] + ':CameraSocketOverride'
        record(override['properties'], 'BoneName')['exported_text'] = 'root'
        record(override['properties'], 'RelativeLocation')['exported_text'] = '(X=12.000000,Y=0.000000,Z=0.000000)'
        changed[5]['asset_objects'].append(override)
        record(changed[5]['asset_properties'], 'Sockets')['exported_text'] = '("/Script/Engine.SkeletalMeshSocket\'' + override['object_path'] + '\'")'
        overridden = {value['name']: value for value in compile_definition(*changed)['sockets']}
        assert overridden['FirstPersonCamera']['bone'] == 'root'
        assert overridden['FirstPersonCamera']['location'] == (0.12, 0.0, 0.0)
        changed = deepcopy(sources)
        component = next(value for value in changed[0]['blueprint_cdo_subobjects'] if value['object_path'].endswith(':Camera'))
        record(component['properties'], 'PostProcessWeight')['exported_text'] = '1.000000'
        assert compile_definition(*changed)['post_process_weight'] == 1.0
        pp = record(changed[2]['asset_properties'], 'PostProcess')['fields']
        next(value for value in pp if value['name'].startswith('bOverride_'))['exported_text'] = 'True'
        try:
            compile_definition(*changed)
        except ValueError as error:
            assert 'post-process' in str(error)
        else:
            raise AssertionError('Active unsupported camera post-process was accepted')
        for name, value in [('CollisionEnabled', 'QueryAndPhysics'), ('bSimulatePhysics', 'True')]:
            changed = deepcopy(sources)
            component = next(item for item in changed[0]['blueprint_cdo_subobjects'] if item['object_path'].endswith(':Camera'))
            record(record(component['properties'], 'BodyInstance')['fields'], name)['exported_text'] = value
            try:
                compile_definition(*changed)
            except ValueError:
                pass
            else:
                raise AssertionError('Unsupported camera physics activation was accepted')
        assert sources == before
        print(json.dumps({'self_test': 'passed', 'raw_sources_unchanged': True}), flush=True)
        return
    if args.output is None or not args.output.name.endswith('.camera_component.ron'):
        parser.error('A camera component target output is required')
    if args.output.resolve().is_relative_to(args.source_directory.resolve()):
        raise ValueError('Target output must be separate from raw source records')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(literal(result) + '\n', encoding='utf-8')
    print(json.dumps({'success': True, 'graph': result['graph'], 'physics_bone': result['physics_body']['bone'], 'collision_enabled': result['collision_enabled']}), flush=True)


if __name__ == '__main__':
    main()
