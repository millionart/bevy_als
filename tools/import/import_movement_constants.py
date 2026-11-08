import argparse
import json
import math
import re
import struct
from pathlib import Path

from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE


NUMBER = r'([0-9]+(?:\.[0-9]*)?(?:[eE][+-]?[0-9]+)?)'


def literal(text, pattern, label):
    matches = re.findall(pattern, text)
    if len(matches) != 1:
        raise ValueError(f'Source value is missing or ambiguous: {label}')
    value = float(matches[0])
    if not math.isfinite(value) or value <= 0:
        raise ValueError(f'Source value is invalid: {label}')
    return value


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def extract(actor, character_cpp, floor_cpp, primitive_cpp, movement_cpp, math_header):
    components = [obj for obj in actor['blueprint_cdo_subobjects']
                  if obj['class_path'] == '/Script/ALS.AlsCharacterMovementComponent']
    if len(components) != 1:
        raise ValueError('Character movement component is missing or ambiguous')
    properties = components[0]['properties']

    def property_number(name):
        values = [float(prop['exported_text']) for prop in properties if prop['name'] == name]
        if len(values) != 1 or not math.isfinite(values[0]) or values[0] <= 0:
            raise ValueError(f'Character property is invalid: {name}')
        return values[0]

    def property_scalar(name):
        values = [float(prop['exported_text']) for prop in properties if prop['name'] == name]
        if len(values) != 1 or not math.isfinite(values[0]):
            raise ValueError(f'Character property is invalid: {name}')
        return values[0]

    def property_bool(name):
        values = [prop['exported_text'] for prop in properties if prop['name'] == name]
        if len(values) != 1 or values[0] not in ('True', 'False'):
            raise ValueError(f'Character property is invalid: {name}')
        return values[0] == 'True'

    scale = SOURCE_LENGTH_TO_RUNTIME_SCALE
    edge = literal(character_cpp, rf'SWEEP_EDGE_REJECT_DISTANCE\s*=\s*{NUMBER}f?\s*;', 'edge distance')
    small = literal(math_header, rf'#define\s+UE_KINDA_SMALL_NUMBER\s+\({NUMBER}f?\)', 'small distance')
    pullback = re.findall(rf'DesiredTimeBack\s*=\s*FMath::Clamp\(\s*{NUMBER}f?\s*,\s*{NUMBER}f?\s*/\s*Dist\s*,\s*{NUMBER}f?\s*/\s*Dist\s*\)\s*\+\s*{NUMBER}f?\s*;', primitive_cpp)
    if len(pullback) != 1:
        raise ValueError('Source pullback expression is missing or ambiguous')
    proportion, minimum, maximum, bias = map(float, pullback[0])
    if not 0 < minimum <= maximum or proportion <= 0 or bias <= 0:
        raise ValueError('Source pullback expression is invalid')
    shrink = literal(floor_cpp, rf'const float ShrinkScale\s*=\s*{NUMBER}f?\s*;', 'floor shrink')
    overlap_shrink = literal(floor_cpp, rf'const float ShrinkScaleOverlap\s*=\s*{NUMBER}f?\s*;', 'floor retry shrink')
    if not 0 < shrink < 1 or not 0 < overlap_shrink < 1:
        raise ValueError('Source floor shrink is invalid')
    return {
        'CHARACTER_MOVEMENT_MASS': property_number('Mass'),
        'STANDING_DOWNWARD_FORCE_SCALE': property_scalar('StandingDownwardForceScale'),
        'ENABLE_PHYSICS_INTERACTION': property_bool('bEnablePhysicsInteraction'),
        'MAX_STEP_HEIGHT': property_number('MaxStepHeight') * scale,
        'MAX_SLOPE_DEGREES': property_number('WalkableFloorAngle'),
        'MIN_FLOOR_DISTANCE': literal(character_cpp, rf'MIN_FLOOR_DIST\s*=\s*{NUMBER}f?\s*;', 'minimum floor distance') * scale,
        'MAX_FLOOR_DISTANCE': literal(character_cpp, rf'MAX_FLOOR_DIST\s*=\s*{NUMBER}f?\s*;', 'floor distance') * scale,
        'EDGE_REJECT_DISTANCE': edge * scale,
        'SWEEP_EDGE_MIN_RADIUS': (edge + small) * scale,
        'PULLBACK_PROPORTION': proportion,
        'PULLBACK_MIN_DISTANCE': minimum * scale,
        'PULLBACK_MAX_DISTANCE': maximum * scale,
        'PULLBACK_TIME_BIAS': bias,
        'FLOOR_SWEEP_SHRINK_FRACTION': f32(1.0 - f32(shrink)),
        'FLOOR_RETRY_SHRINK_FRACTION': f32(1.0 - f32(overlap_shrink)),
        'MAX_SPEED_SQUARED_RATIO': literal(movement_cpp, rf'OverVelocityPercent\s*=\s*{NUMBER}f?\s*;', 'speed tolerance'),
        'BRAKE_TO_STOP_VELOCITY': literal(character_cpp, rf'BRAKE_TO_STOP_VELOCITY\s*=\s*{NUMBER}f?\s*;', 'braking stop velocity') * scale,
        'BRAKING_SMALL_VELOCITY_SQUARED': small * scale * scale,
        'MIN_MOVEMENT_DELTA_SECONDS': literal(character_cpp, rf'MIN_TICK_TIME\s*=\s*{NUMBER}f?\s*;', 'minimum movement delta'),
        'STAND_UP_SWEEP_INFLATION': small * literal(character_cpp, rf'const float SweepInflation\s*=\s*UE_KINDA_SMALL_NUMBER\s*\*\s*{NUMBER}f?\s*;', 'stand-up inflation') * scale,
        'STAND_UP_MIN_FLOOR_DISTANCE': small * literal(character_cpp, rf'const float MinFloorDist\s*=\s*UE_KINDA_SMALL_NUMBER\s*\*\s*{NUMBER}f?\s*;', 'stand-up floor distance') * scale,
        'MAX_DEPENETRATION_WITH_GEOMETRY': property_number('MaxDepenetrationWithGeometry') * scale,
        'MAX_DEPENETRATION_WITH_PAWN': property_number('MaxDepenetrationWithPawn') * scale,
        'PENETRATION_PULLBACK_DISTANCE': literal(movement_cpp, rf'float PenetrationPullbackDistance\s*=\s*{NUMBER}f?\s*;', 'penetration pullback') * scale,
        'PENETRATION_OVERLAP_INFLATION': literal(movement_cpp, rf'float PenetrationOverlapCheckInflation\s*=\s*{NUMBER}f?\s*;', 'penetration overlap inflation') * scale,
    }


def render(values):
    return ''.join(f'pub(crate) const {name}: bool = {str(value).lower()};\n' if isinstance(value, bool)
                   else f'pub(crate) const {name}: f32 = {value!r};\n' for name, value in values.items())


def self_test():
    actor = {'blueprint_cdo_subobjects': [{'class_path': '/Script/ALS.AlsCharacterMovementComponent',
        'properties': [{'name': 'MaxStepHeight', 'exported_text': '45.000000'},
                       {'name': 'WalkableFloorAngle', 'exported_text': '44.765083'},
                       {'name': 'MaxDepenetrationWithGeometry', 'exported_text': '500.000000'},
                       {'name': 'MaxDepenetrationWithPawn', 'exported_text': '100.000000'},
                       {'name': 'Mass', 'exported_text': '100.000000'},
                       {'name': 'StandingDownwardForceScale', 'exported_text': '1.000000'},
                       {'name': 'bEnablePhysicsInteraction', 'exported_text': 'True'}]}]}
    character = 'SWEEP_EDGE_REJECT_DISTANCE = 0.15f; MIN_FLOOR_DIST = 1.9f; MAX_FLOOR_DIST = 2.4f; BRAKE_TO_STOP_VELOCITY = 10.f; MIN_TICK_TIME = 1e-6f; const float ShrinkScale = 0.9f; const float ShrinkScaleOverlap = 0.1f; const float SweepInflation = UE_KINDA_SMALL_NUMBER * 10.f; const float MinFloorDist = UE_KINDA_SMALL_NUMBER * 10.f;'
    primitive = 'const float DesiredTimeBack = FMath::Clamp(0.1f, 0.1f/Dist, 1.f/Dist) + 0.001f;'
    movement = 'const float OverVelocityPercent = 1.01f; float PenetrationPullbackDistance = 0.125f; float PenetrationOverlapCheckInflation = 0.100f;'
    header = '#define UE_KINDA_SMALL_NUMBER (1.e-4f)'
    values = extract(actor, character, character, primitive, movement, header)
    assert values['CHARACTER_MOVEMENT_MASS'] == 100.0
    assert values['STANDING_DOWNWARD_FORCE_SCALE'] == 1.0
    assert values['ENABLE_PHYSICS_INTERACTION'] is True
    changed_actor = json.loads(json.dumps(actor))
    changed_properties = changed_actor['blueprint_cdo_subobjects'][0]['properties']
    next(prop for prop in changed_properties if prop['name'] == 'StandingDownwardForceScale')['exported_text'] = '0.0'
    next(prop for prop in changed_properties if prop['name'] == 'bEnablePhysicsInteraction')['exported_text'] = 'False'
    changed_values = extract(changed_actor, character, character, primitive, movement, header)
    assert changed_values['STANDING_DOWNWARD_FORCE_SCALE'] == 0.0
    assert changed_values['ENABLE_PHYSICS_INTERACTION'] is False
    assert 'pub(crate) const ENABLE_PHYSICS_INTERACTION: bool = false;' in render(changed_values)
    changed_properties.append({'name': 'Mass', 'exported_text': '100.0'})
    try:
        extract(changed_actor, character, character, primitive, movement, header)
    except ValueError:
        pass
    else:
        raise AssertionError('Duplicate character mass must block generation')
    assert values['MAX_STEP_HEIGHT'] == 0.45
    assert values['MIN_FLOOR_DISTANCE'] == 0.019
    assert values['EDGE_REJECT_DISTANCE'] == 0.0015
    assert abs(values['SWEEP_EDGE_MIN_RADIUS'] - 0.001501) < 1e-12
    assert values['PULLBACK_MIN_DISTANCE'] == 0.001
    assert values['PULLBACK_MAX_DISTANCE'] == 0.01
    assert values['MAX_SPEED_SQUARED_RATIO'] == 1.01
    assert values['BRAKE_TO_STOP_VELOCITY'] == 0.1
    assert abs(values['BRAKING_SMALL_VELOCITY_SQUARED'] - 1e-8) < 1e-20
    assert values['MIN_MOVEMENT_DELTA_SECONDS'] == 1e-6
    changed_stop = extract(actor, character.replace('BRAKE_TO_STOP_VELOCITY = 10.f', 'BRAKE_TO_STOP_VELOCITY = 20.f'), character, primitive, movement, header)
    assert changed_stop['BRAKE_TO_STOP_VELOCITY'] == 0.2
    assert values['STAND_UP_SWEEP_INFLATION'] == 1e-5
    assert values['STAND_UP_MIN_FLOOR_DISTANCE'] == 1e-5
    assert values['MAX_DEPENETRATION_WITH_GEOMETRY'] == 5.0
    assert values['MAX_DEPENETRATION_WITH_PAWN'] == 1.0
    assert values['PENETRATION_PULLBACK_DISTANCE'] == 0.00125
    assert values['PENETRATION_OVERLAP_INFLATION'] == 0.001
    assert values['FLOOR_SWEEP_SHRINK_FRACTION'] == f32(1.0 - f32(0.9))
    changed = extract(actor, character.replace('0.15f', '0.25f'), character, primitive.replace('1.f/Dist', '2.f/Dist'), movement, header)
    assert changed['EDGE_REJECT_DISTANCE'] == 0.0025
    assert changed['PULLBACK_MAX_DISTANCE'] == 0.02
    independent = extract(actor, character.replace('SweepInflation = UE_KINDA_SMALL_NUMBER * 10.f', 'SweepInflation = UE_KINDA_SMALL_NUMBER * 20.f'), character, primitive, movement, header)
    assert independent['STAND_UP_SWEEP_INFLATION'] == 2e-5
    assert independent['STAND_UP_MIN_FLOOR_DISTANCE'] == 1e-5
    for broken in [character.replace('MAX_FLOOR_DIST', 'MISSING'), character + ' MAX_FLOOR_DIST = 4.f;', character.replace('SweepInflation', 'MISSING'), character + ' const float MinFloorDist = UE_KINDA_SMALL_NUMBER * 10.f;', character.replace('BRAKE_TO_STOP_VELOCITY', 'MISSING'), character + ' MIN_TICK_TIME = 1e-5f;']:
        try:
            extract(actor, broken, character, primitive, movement, header)
        except ValueError:
            pass
        else:
            raise AssertionError('Invalid source must block generation')
    terminal_pattern = rf'DefaultTerminalVelocity\(\s*{NUMBER}f?\s*\)'
    assert literal('DefaultTerminalVelocity(4000.f)', terminal_pattern, 'terminal') * SOURCE_LENGTH_TO_RUNTIME_SCALE == 40.0
    assert literal('DefaultTerminalVelocity(8000.f)', terminal_pattern, 'terminal') * SOURCE_LENGTH_TO_RUNTIME_SCALE == 80.0
    for broken in ['Missing(4000.f)', 'DefaultTerminalVelocity(4000.f) DefaultTerminalVelocity(4000.f)']:
        try:
            literal(broken, terminal_pattern, 'terminal')
        except ValueError:
            pass
        else:
            raise AssertionError('Invalid terminal source must block generation')
    assert literal('TEXT("p.EncroachEpsilon"), 0.15f,', rf'\"p\.EncroachEpsilon\"\s*\)\s*,\s*{NUMBER}f?\s*,', 'spawn epsilon') * SOURCE_LENGTH_TO_RUNTIME_SCALE == 0.0015
    assert literal('PawnRadius - GetPerchRadiusThreshold(), 0.11f, PawnRadius', rf'PawnRadius - GetPerchRadiusThreshold\(\),\s*{NUMBER}f?\s*,\s*PawnRadius', 'perch minimum') * SOURCE_LENGTH_TO_RUNTIME_SCALE == 0.0011
    assert len(render(values).splitlines()) == 25
    print('Movement constants self-test PASS')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--actor-native', type=Path)
    parser.add_argument('--engine-root', type=Path)
    parser.add_argument('--als-source', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--report', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    if not all((args.actor_native, args.engine_root, args.als_source, args.output)):
        parser.error('Actor native record, engine root, ALS source, and output are required')
    engine = args.engine_root / 'Engine/Source/Runtime'
    actor = json.loads(args.actor_native.read_text(encoding='utf-8-sig'))
    character = (engine / 'Engine/Private/Components/CharacterMovementComponent.cpp').read_text(encoding='utf-8-sig')
    floor = (args.als_source / 'ALS/Private/AlsCharacterMovementComponent.cpp').read_text(encoding='utf-8-sig')
    values = extract(actor, character, floor,
        (engine / 'Engine/Private/Components/PrimitiveComponent.cpp').read_text(encoding='utf-8-sig'),
        (engine / 'Engine/Private/Components/MovementComponent.cpp').read_text(encoding='utf-8-sig'),
        (engine / 'Core/Public/Math/UnrealMathUtility.h').read_text(encoding='utf-8-sig'))
    physics = (engine / 'PhysicsCore/Private/PhysicsSettingsCore.cpp').read_text(encoding='utf-8-sig')
    values['DEFAULT_TERMINAL_VELOCITY'] = literal(physics, rf'DefaultTerminalVelocity\(\s*{NUMBER}f?\s*\)', 'default terminal velocity') * SOURCE_LENGTH_TO_RUNTIME_SCALE
    level = (engine / 'Engine/Private/LevelActor.cpp').read_text(encoding='utf-8-sig')
    values['SPAWN_ENCROACH_EPSILON'] = literal(level, rf'\"p\.EncroachEpsilon\"\s*\)\s*,\s*{NUMBER}f?\s*,', 'spawn encroach epsilon') * SOURCE_LENGTH_TO_RUNTIME_SCALE
    values['MIN_PERCH_RADIUS'] = literal(character, rf'PawnRadius - GetPerchRadiusThreshold\(\),\s*{NUMBER}f?\s*,\s*PawnRadius', 'minimum perch radius') * SOURCE_LENGTH_TO_RUNTIME_SCALE
    math = (engine / 'Core/Public/Math/UnrealMathUtility.h').read_text(encoding='utf-8-sig')
    values['SMALL_MOVEMENT_DISTANCE'] = literal(math, rf'#define\s+UE_KINDA_SMALL_NUMBER\s+\(?{NUMBER}f?\)?', 'small movement distance') * SOURCE_LENGTH_TO_RUNTIME_SCALE
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(render(values), encoding='utf-8')
    result = {'success': True, 'source_actor': actor['asset_path'], 'length_scale': SOURCE_LENGTH_TO_RUNTIME_SCALE, 'runtime_values': values}
    if args.report:
        args.report.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
