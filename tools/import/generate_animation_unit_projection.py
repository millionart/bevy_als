import argparse
import hashlib
import json
import struct
from pathlib import Path

from audit_animation_transform_conversion import accessor_values, load_gltf


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def generate(raw_path, compressed_path, gltf_path, output_path):
    raw = json.loads(raw_path.read_text())
    compressed = json.loads(compressed_path.read_text())
    raw_probe = raw['sequence_pose_probe']
    compressed_probe = compressed['sequence_pose_probe']
    if raw['asset_path'] != compressed['asset_path'] or raw_probe['required_bones_mesh'] != compressed_probe['required_bones_mesh']:
        raise ValueError('source sequence or mesh differs')
    if raw_probe['sampling_mode'] != 'raw_absolute_pose' or compressed_probe['sampling_mode'] != 'animation_pose':
        raise ValueError('source sampling modes differ')
    raw_frames = raw_probe['frames']
    compressed_frames = compressed_probe['frames']
    if not raw_frames or len(raw_frames) != len(compressed_frames):
        raise ValueError('source frame counts differ')
    times = [frame['time'] for frame in raw_frames]
    if times != [frame['time'] for frame in compressed_frames]:
        raise ValueError('source frame times differ')
    source_frames = []
    for first, second in zip(raw_frames, compressed_frames):
        if first['pose_evaluation_mode'] != 'raw_model' or second['pose_evaluation_mode'] != 'compressed':
            raise ValueError('source pose evaluation modes differ')
        a = {bone['name']: bone['local_transform']['translation'] for bone in first['bones']}
        b = {bone['name']: bone['local_transform']['translation'] for bone in second['bones']}
        if len(a) != len(first['bones']) or len(b) != len(second['bones']) or a.keys() != b.keys():
            raise ValueError('source bone identities differ')
        source_frames.append((a, b))
    document, original = load_gltf(gltf_path)
    sources = {path.resolve() for path in (raw_path, compressed_path, gltf_path, gltf_path.parent / document['buffers'][0]['uri'])}
    if output_path.resolve() in sources or output_path.with_suffix('.bin').resolve() in sources or len(document['animations']) != 1:
        raise ValueError('output or animation count is invalid')
    animation = document['animations'][0]
    data = bytearray(original)
    changes = []
    matching = 0
    different = 0
    used = set()
    updated_accessors = set()
    for channel in animation['channels']:
        target = channel['target']
        if target.get('path') != 'translation':
            continue
        name = document['nodes'][target['node']]['name']
        if name in used or name not in source_frames[0][0]:
            raise ValueError('translation bone identity is invalid')
        used.add(name)
        sampler = animation['samplers'][channel['sampler']]
        sampled_times = [value[0] for value in accessor_values(document, original, sampler['input'])]
        values = accessor_values(document, original, sampler['output'])
        if sampled_times != times or len(values) != len(times):
            raise ValueError('translation sample times differ')
        accessor = document['accessors'][sampler['output']]
        view = document['bufferViews'][accessor['bufferView']]
        if accessor.get('type') != 'VEC3' or view.get('buffer', 0) != 0:
            raise ValueError('translation accessor is invalid')
        start = view.get('byteOffset', 0) + accessor.get('byteOffset', 0)
        stride = view.get('byteStride', 12)
        for frame, value in enumerate(values):
            first, second = source_frames[frame]
            for axis, source_axis in enumerate(('x', 'z', 'y')):
                raw_value = first[name][source_axis]
                compressed_value = second[name][source_axis]
                if raw_value != compressed_value:
                    different += 1
                    continue
                matching += 1
                exported = f32(f32(compressed_value) * f32(0.01))
                if value[axis] != exported:
                    raise ValueError('source and exported translation differ')
                projected = f32(compressed_value * 0.01)
                if value[axis] != projected:
                    offset = start + frame * stride + axis * 4
                    struct.pack_into('<f', data, offset, projected)
                    changes.append({'frame': frame, 'bone': name, 'source_axis': source_axis,
                                    'byte_offset': offset, 'source_cm': compressed_value,
                                    'exported': value[axis], 'runtime': projected})
                    updated_accessors.add(sampler['output'])
        if len(used) % 20 == 0:
            print(f'Projected {len(used)} source bones', flush=True)
    if used != source_frames[0][0].keys():
        raise ValueError('source and exported bone sets differ')
    for index in updated_accessors:
        accessor = document['accessors'][index]
        values = accessor_values(document, data, index)
        if 'min' in accessor:
            accessor['min'] = [min(row[axis] for row in values) for axis in range(3)]
        if 'max' in accessor:
            accessor['max'] = [max(row[axis] for row in values) for axis in range(3)]
    document['buffers'][0]['uri'] = output_path.with_suffix('.bin').name
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(document, indent=2) + '\n')
    output_path.with_suffix('.bin').write_bytes(data)
    return {
        'source_asset': raw['asset_path'], 'mesh_asset': raw_probe['required_bones_mesh'],
        'frames': len(times), 'bones': len(used), 'equal_source_scalars': matching,
        'different_source_scalars': different, 'changed_scalars': len(changes), 'changes': changes,
        'input_gltf_sha256': hashlib.sha256(gltf_path.read_bytes()).hexdigest(),
        'input_buffer_sha256': hashlib.sha256(original).hexdigest(),
        'output_gltf_sha256': hashlib.sha256(output_path.read_bytes()).hexdigest(),
        'output_buffer_sha256': hashlib.sha256(data).hexdigest(),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--raw', type=Path, required=True)
    parser.add_argument('--compressed', type=Path, required=True)
    parser.add_argument('--gltf', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--descriptor', type=Path, required=True)
    parser.add_argument('--asset-root', type=Path, default=Path('assets'))
    args = parser.parse_args()
    source_document = json.loads(args.gltf.read_text())
    source_buffer = args.gltf.parent / source_document['buffers'][0]['uri']
    protected = {path.resolve() for path in (args.raw, args.compressed, args.gltf, source_buffer)}
    if args.descriptor.resolve() in protected or args.report.resolve() in protected:
        raise ValueError('output would overwrite source data')
    report = generate(args.raw, args.compressed, args.gltf, args.output)
    source = f'path: "{args.gltf.relative_to(args.asset_root).as_posix()}"'
    target = f'path: "{args.output.relative_to(args.asset_root).as_posix()}"'
    descriptor = args.descriptor.read_text()
    if descriptor.count(source) == 1:
        args.descriptor.write_text(descriptor.replace(source, target))
    elif descriptor.count(target) != 1:
        raise ValueError('animation descriptor source is invalid')
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(f"Projected {report['changed_scalars']} translation scalars", flush=True)


if __name__ == '__main__':
    main()
