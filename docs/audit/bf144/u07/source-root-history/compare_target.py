import argparse
import gzip
import json
import math
import struct
from collections import Counter
from pathlib import Path


def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]


def vector(value, scale=1):
    return tuple(value[axis] * scale for axis in ('x', 'z', 'y'))


def native_transform(value, length_scale, projection):
    rotation = value['rotation']
    return (
        tuple(map(projection, vector(value['translation'], length_scale))),
        tuple(map(projection, (-rotation['x'], -rotation['z'], -rotation['y'], rotation['w']))),
        tuple(map(projection, vector(value['scale']))),
    )


def pose_rows(source, target, transform, projection):
    result = Counter()
    maximum = [0, 0, 0]
    first = None
    missing = set(source) - set(target)
    for name in sorted(source.keys() & target.keys()):
        a, b = source[name], transform(target[name])
        for channel, (left, right) in enumerate(zip(a, b)):
            if right is None:
                result[f'channel_{channel}_missing'] += 1
                continue
            right = tuple(map(projection, right))
            if channel == 1 and sum(x*y for x,y in zip(left,right)) < 0:
                right = tuple(-value for value in right)
            error = max(abs(x-y) for x,y in zip(left,right))
            maximum[channel] = max(maximum[channel], error)
            if error:
                result[f'channel_{channel}_different'] += 1
                if first is None:
                    first = {'bone': name, 'channel': channel, 'source': left, 'target': right}
    return result, maximum, first, sorted(missing), sorted(set(target) - set(source))


def read_json(path):
    if path.suffix == ".gz":
        with gzip.open(path, "rb") as file:
            return json.load(file)
    return json.loads(path.read_bytes())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('source', type=Path)
    parser.add_argument('target', type=Path)
    args = parser.parse_args()
    native = read_json(args.source)['character_animation_probe']['cases'][0]
    source = [native['initialization'], *native['frames']]
    target = read_json(args.target)
    cold = [row for row in target if row['active'] is None]
    active = [row for row in target if row['active'] is not None]
    assert len(active) == 240 and [row['active'] for row in active] == list(range(240))
    assert all(a['arena'] < b['arena'] for a,b in zip(target, target[1:]))
    source_cold = len(source) - len(active)
    pairs = [('cold', i, a, b) for i,(a,b) in enumerate(zip(source[:source_cold],cold))]
    pairs += [('active', i, a, b) for i,(a,b) in enumerate(zip(source[source_cold:],active))]
    summary = {'source_cold':source_cold,'target_cold':len(cold),'target_active':len(active),'pairs':{}}
    for phase in ('cold','active'):
        stats = {'counts':Counter(), 'stages':{}}
        for _,index,a,b in (row for row in pairs if row[0] == phase):
            pose = a['pose_boundaries']
            stats['counts']['frames'] += 1
            if a.get('world_delta_seconds') is not None and b['inputs'] is not None:
                expected = f32(a['world_delta_seconds'])
                stats['counts']['matching_world_delta'] += expected == f32(b['inputs']['delta_ns']/1e9)
            if a.get('movement_after_tick') is not None:
                move = a['movement_after_tick']
                expected = {'MOVE_Walking':'Grounded','MOVE_Falling':'Air'}.get(move['movement_mode_name'])
                stats['counts']['matching_mode'] += expected == b['mode']
                stats['counts']['same_move_intent'] += math.dist(vector(move['last_input_world']), b['movement_input']) < 1e-6
            stats['counts']['matching_position_within_0_1mm'] += math.dist(vector(a['actor_transform']['translation'], .01), b['position']) <= .0001
            for key,out,precision,scale,extract in (
                ('rig_source', b['poses']['rig_source'], f32, .01, lambda value: value),
                ('rig_output', b['poses']['rig_output'], f32, .01, lambda value: value),
                ('root_result', b['root'], f32, .01, lambda value: value),
                ('rig_local', {'bones':b['rig_local']}, float, 1.0, lambda v:(v['translation'],v['rotation'],v['scale'])),
            ):
                stage = stats['stages'].setdefault(key, {'counts':Counter(),'max_error':[0,0,0],'first':None})
                if out is None:
                    stage['counts']['missing_frames'] += 1
                    continue
                packet = pose['rig_output' if key=='rig_local' else key]['pose']
                source_bones = {r['name']:native_transform(r['local_transform'], scale, precision) for r in packet['bones']}
                diff, maximum, first, missing, extra = pose_rows(source_bones, out['bones'], extract, precision)
                stage['counts'].update(diff)
                stage['counts']['frames_with_missing_bones'] += bool(missing)
                stage['counts']['frames_with_extra_bones'] += bool(extra)
                stage['counts']['exact_frames_on_shared_bones'] += not diff and not missing
                stage['max_error'] = list(map(max,stage['max_error'],maximum))
                if stage['first'] is None and (first is not None or missing or extra):
                    stage['first'] = {'index':index,'arena':b['arena'],'first_numeric':first,'missing':missing,'extra':extra}
            left = {key.casefold():f32(value) for key,value in pose['root_result']['pose']['curves'].items()}
            right = {key.casefold():f32(value) for key,value in (b['curves'] or {}).items()}
            stats['counts']['exact_root_curves'] += left == right
            if left != right and 'first_curve_difference' not in stats:
                stats['first_curve_difference'] = {'index':index, 'values':{key:[left.get(key),right.get(key)]
                    for key in sorted(left.keys() | right.keys()) if left.get(key)!=right.get(key)}}
        summary['pairs'][phase] = stats
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
