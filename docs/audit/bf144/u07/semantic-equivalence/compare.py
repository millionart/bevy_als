from pathlib import Path
import collections
import gzip
import json
import math
import re
import struct
import sys

root = Path('docs/audit/bf144/u07')
assets = Path('assets/ALS/Character/AnimationInstances')


def load(path):
    return json.loads(gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes())


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def xyz(value, scale=1):
    return tuple(value[axis] * scale for axis in 'xzy')


def rotation_delta(source, target):
    left = tuple(value / math.hypot(*source) for value in source)
    right = tuple(value / math.hypot(*target) for value in target)
    chord = min(math.dist(left, right), math.dist(left, [-value for value in right]))
    return math.degrees(4 * math.asin(min(1.0, chord / 2.0)))


def curve_values(pose):
    return {key.casefold(): value for key, value in pose['curves'].items()}


def clamp(value):
    return min(1.0, max(0.0, value))


def plant_branch(value):
    if value <= -0.5:
        return 1
    if value > 0.5:
        return 2
    return 3 if value <= 0.0 else 4


def direction_branch(value):
    if abs(value) <= 75.0:
        return 'Forward'
    if abs(value) <= 115.0:
        return 'Left' if value < 0 else 'Right'
    return 'Backward'


def source_curve_consumers(curves):
    left_ik = clamp(curves.get('footleftik', 0.0))
    right_ik = clamp(curves.get('footrightik', 0.0))
    sprint = clamp(curves.get('sprintblock', 0.0))
    return {
        'stop_foot_selection': plant_branch(curves.get('footplanted', 0.0)),
        'allow_transitions': curves.get('allowtransitions', 0.0) >= f32(1.0 - 1e-5),
        'left_ik_relevant': left_ik > f32(1e-5),
        'right_ik_relevant': right_ik > f32(1e-5),
        'left_lock_relevant': f32(left_ik * clamp(curves.get('footleftlock', 0.0))) > f32(1e-5),
        'right_lock_relevant': f32(right_ik * clamp(curves.get('footrightlock', 0.0))) > f32(1e-5),
        'sprint_blend_children': (sprint < f32(1.0 - 1e-5), sprint > f32(1e-5)),
        'footstep_default_volume_relevant': f32(1.0 - clamp(curves.get('footstepsoundblock', 0.0))) > f32(1e-5),
    }


def record_max(mapping, key, value, frame, bone=None):
    if key not in mapping or value > mapping[key]['absolute_error']:
        row = {'absolute_error': value, 'frame': frame}
        if bone is not None:
            row['bone'] = bone
        mapping[key] = row


def machine_labels(path):
    contents = path.read_text()
    states = dict(re.findall(r'\(id: "([0-9a-f-]+)", label: "([^"]+)"', contents))
    transitions = {
        identifier: (states[start], states[end])
        for identifier, start, end in re.findall(
            r'\(id: "([0-9a-f-]+)", source: "([0-9a-f-]+)", target: "([0-9a-f-]+)"', contents
        )
    }
    return states, transitions


def native_machine(frame, name):
    instance = next(item for item in frame['linked_instances'] if f'AB_Als_{name}.' in item['class'])
    return next(machine for machine in instance['machines'] if machine['property'] == 'AnimGraphNode_StateMachine')


def target_machine(frame, name):
    return next(state for machine in frame['source_machines'] for state in machine
                if state['name'] == f'{name} States\nState Machine')


def grounded_aliases():
    native = load(root / 'grounded-roll-slot/grounded.native.json.gz')
    graph = next(graph for graph in native['graphs'] if graph['name'] == 'Grounded States')
    names = {node['object_path'].rsplit('.', 1)[-1]: node['title'] for node in graph['nodes']}
    return {
        node['title']: {names[key] for key in re.findall(r'AnimStateNode_\d+', prop['exported_text'])}
        for node in graph['nodes'] for prop in node['properties'] if prop['name'] == 'AliasedStateNodes'
    }


def compare_machine(source, target, states, transitions, aliases):
    guid = re.search(r'[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}', target['state']).group()
    if guid in states:
        destination = states[guid]
    else:
        previous, destination = transitions[guid]
        active = source['active_transitions']
        assert active
        weights = {value['index']: value['name'] for value in source['state_weights']}
        assert any(
            weights[value['previous_state']] in aliases.get(previous, {previous})
            and weights[value['next_state']] == destination
            and value['elapsed'] == target['elapsed']
            for value in active
        ), (previous, destination, source['state'], active)
    assert destination == source['state'], (destination, source['state'])
    assert target['state_time'] == source['elapsed'], (target, source)


def main():
    target = load(Path(sys.argv[1]))
    source_case = load(root / 'crouch-warm-120/source.native.json.gz')['character_animation_probe']['cases'][0]
    source = [source_case['initialization'], *source_case['frames']]
    assert len(source) == len(target) == 89
    assert [row['active'] for row in target] == list(range(89))
    assert all(b['arena'] == a['arena'] + 1 for a, b in zip(target, target[1:]))
    assert all(row['startup_ready'] and row['rig_updates'] > 0 for row in target)
    states = {
        'Locomotion': machine_labels(assets / 'AB_Als_Locomotion/AnimGraph/SM9_Locomotion_States.fsm.ron'),
        'Grounded': machine_labels(assets / 'AB_Als_Grounded/AnimGraph/SM11_Grounded_States.fsm.ron'),
    }
    aliases = {'Locomotion': {}, 'Grounded': grounded_aliases()}
    movement_names = {'MOVE_Walking': 'Grounded', 'MOVE_Falling': 'Air'}
    curve_differences = collections.Counter()
    curve_maxima = {}
    positions = {}
    poses = collections.defaultdict(dict)
    pose_rows = collections.Counter()
    mode_counts = collections.Counter()
    mode_transitions = []
    machine_frames = collections.Counter()
    gate_frames = collections.Counter()
    for frame, (left, right) in enumerate(zip(source, target, strict=True)):
        record_max(positions, 'position_m', math.dist(xyz(left['actor_transform']['translation'], 0.01), right['position']), frame)
        record_max(positions, 'velocity_mps', math.dist(xyz(left['actor_velocity_cm_s'], 0.01), right['velocity']), frame)
        if frame:
            move = left['movement_after_tick']
            mode = movement_names[move['movement_mode_name']]
            assert right['mode'] == mode
            mode_counts[mode] += 1
            if right['mode'] != target[frame - 1]['mode']:
                mode_transitions.append([frame, target[frame - 1]['mode'], mode])
            assert f32(right['inputs']['delta_ns'] / 1e9) == f32(left['world_delta_seconds'])
            record_max(positions, 'movement_input', math.dist(xyz(move['last_input_world']), right['movement_input']), frame)
            if right['source_machines']:
                for name in states:
                    compare_machine(native_machine(left, name), target_machine(right, name), *states[name], aliases[name])
                    machine_frames[name] += 1
        else:
            assert right['mode'] == 'Grounded'
        for stage in ('rig_source', 'rig_output', 'root_result'):
            source_boundary = left['pose_boundaries'][stage]
            assert source_boundary['attribute_count'] == 0, (frame, stage)
            source_pose = source_boundary['pose']
            target_pose = right['root'] if stage == 'root_result' else right['poses'][stage]
            source_bones = {bone['name']: bone['local_transform'] for bone in source_pose['bones']}
            names = target_pose['bones'].keys()
            assert source_bones.keys() <= names
            assert names - source_bones.keys() <= {'SKM_Als'}
            assert bool(source_pose.get('root_motion')) == bool(target_pose['root_motion'])
            for bone, native in source_bones.items():
                t, q, scale = target_pose['bones'][bone]
                assert all(math.isfinite(value) for values in (t, q, scale) for value in values)
                native_q = native['rotation']
                native_q = (-native_q['x'], -native_q['z'], -native_q['y'], native_q['w'])
                record_max(poses[stage], 'translation_m', math.dist(xyz(native['translation'], 0.01), t), frame, bone)
                record_max(poses[stage], 'rotation_degrees', rotation_delta(native_q, q), frame, bone)
                record_max(poses[stage], 'scale', math.dist(xyz(native['scale']), scale), frame, bone)
                pose_rows[stage] += 1
        left_curves = curve_values(left['pose_boundaries']['root_result']['pose'])
        for name in ('rotationyawoffset', 'rotationyawspeed'):
            if name in left_curves:
                left_curves[name] *= -1
        right_curves = curve_values(right)
        assert left_curves.keys() == right_curves.keys(), (frame, left_curves.keys() ^ right_curves.keys())
        for key in left_curves:
            a, b = f32(left_curves[key]), f32(right_curves[key])
            assert math.isfinite(a) and math.isfinite(b)
            if a != b:
                curve_differences[key] += 1
                record_max(curve_maxima, key, abs(a - b), frame)
        target_consumers = source_curve_consumers(right_curves)
        for name, expected in source_curve_consumers(left_curves).items():
            assert target_consumers[name] == expected, (frame, name)
            gate_frames[name] += 1
        if 7 <= frame <= 10:
            assert left_curves['sprintblock'] > 0 and right_curves['sprintblock'] > 0
            assert clamp(left_curves['footleftik']) == clamp(right_curves['footleftik']) == 1.0
        if 12 <= frame < 72:
            heading = math.degrees(math.atan2(right['movement_input'][2], right['movement_input'][0]))
            native_input = left['movement_after_tick']['last_input_world']
            native_heading = math.degrees(math.atan2(native_input['y'], native_input['x']))
            assert direction_branch(heading) == direction_branch(native_heading) == 'Forward'
            gate_frames['forward_input'] += 1
    assert machine_frames == {'Locomotion': 16, 'Grounded': 16}
    print(json.dumps({
        'ordinary_frames': len(target) - 1,
        'modes': mode_counts, 'mode_transitions': mode_transitions, 'machine_frames': machine_frames,
        'matching_consumer_classification': gate_frames, 'world_maxima': positions,
        'root_curve_differences': curve_differences, 'root_curve_maxima': curve_maxima,
        'source_attributes': 'empty in all three pose boundaries', 'pose_rows': pose_rows, 'pose_maxima': poses,
    }, indent=2))


if __name__ == '__main__':
    main()
