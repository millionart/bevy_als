import gzip
import json
import math
import sys
from pathlib import Path


def read(path):
    path = Path(path)
    with (gzip.open(path, 'rt') if path.suffix == '.gz' else path.open()) as file:
        return json.load(file)


def multiply(a, b):
    x, y, z, w = a
    bx, by, bz, bw = b
    return (w * bx + x * bw + y * bz - z * by,
            w * by - x * bz + y * bw + z * bx,
            w * bz + x * by - y * bx + z * bw,
            w * bw - x * bx - y * by - z * bz)


def relative(parent, child):
    x, y, z, w = parent
    return multiply((-x, -y, -z, w), child)


def degrees(a, b):
    cosine = abs(sum(x * y for x, y in zip(a, b))) / (math.hypot(*a) * math.hypot(*b))
    return math.degrees(2 * math.acos(min(1, cosine)))


def source_rotation(transform):
    q = transform['rotation']
    return (-q['x'], -q['z'], -q['y'], q['w'])


source = read(sys.argv[1])['character_animation_probe']['cases']
source_frames = [row for row in source[1]['frames'] if 'GetUp' in row['montage'] and row['linked_instances'][0]['montage_evaluation']]
source_clip_time = lambda row: row['linked_instances'][0]['montage_evaluation'][0]['position']
parents = {'thigh_l': 'pelvis', 'calf_l': 'thigh_l', 'foot_l': 'calf_l',
           'thigh_r': 'pelvis', 'calf_r': 'thigh_r', 'foot_r': 'calf_r'}
result = {'source': [], 'targets': []}
for case in source:
    frames = [row for row in case['frames'] if 'GetUp' in row['montage']]
    result['source'].append({'case': case['name'], 'get_up_frames': len(frames),
        'modes': sorted({row['movement_after_tick']['movement_mode_name'] for row in frames}),
        'mesh_height_cm': sorted({row['mesh_to_world']['translation']['z'] for row in frames})})
for path in sys.argv[2:]:
    report = read(path)
    window = next(window for window in report['motion_windows'] if window['label'] == 'get_up_back')
    rows = []
    for entry in window['actor_yaw_trace']:
        row = min(report['pose_samples'], key=lambda row: abs(row['elapsed_secs'] - entry['elapsed_secs']))
        if abs(row['elapsed_secs'] - entry['elapsed_secs']) > 1e-7:
            continue
        time = entry['animation_clip_time']
        reference = min(source_frames, key=lambda source: abs(source_clip_time(source) - time))
        if abs(source_clip_time(reference) - time) > 0.001:
            continue
        errors = {}
        for name, parent in parents.items():
            native = relative(source_rotation(reference[parent]), source_rotation(reference[name]))
            target = relative(row['bones'][parent]['rotation'], row['bones'][name]['rotation'])
            errors[name] = degrees(native, target)
        right = row['foot_ik']['right']
        mesh_y = row['ragdoll_root_space']['player_world_translation'][1]
        rig_y = right['solve_input']['armature_world_trs'][0][1] if right['solve_input'] else None
        rows.append({'time': time, 'mode': row['locomotion'], 'mesh_y_m': mesh_y,
            'rig_y_m': rig_y, 'right_ik_effector_error_m': right['effector_error'],
            'local_rotation_error_degrees': errors})
    assert rows
    result['targets'].append({'report': Path(path).parent.parent.name + '/' + Path(path).parent.name, 'rows': rows})
print(json.dumps(result, indent=2))
