import argparse
import json
import math
import sys
from copy import deepcopy
from pathlib import Path

from audit_animation_transform_conversion import accessor_values, load_gltf
from coordinate_basis import source_direction_to_runtime, source_point_to_runtime, source_rotation_to_runtime


def validate(document, native):
    bones = native['skeletal_mesh']['bones']
    skin = document['skins'][0]
    if len(document['skins']) != 1 or len(skin['joints']) != len(bones):
        raise ValueError('Source and target joint counts differ')
    parents = {child: index for index, node in enumerate(document['nodes']) for child in node.get('children', [])}
    for bone, index in zip(bones, skin['joints']):
        node = document['nodes'][index]
        if node.get('name') != bone['name'] or 'matrix' in node:
            raise ValueError('Source and target bone identity or transform encoding differs')
        parent = bone['parent_index']
        if (parent >= 0 and parents.get(index) != skin['joints'][parent]) or (parent < 0 and parents.get(index) in skin['joints']):
            raise ValueError('Source and target bone parents differ')
        source = bone['local_transform']
        expected = source_point_to_runtime([source['translation'][axis] for axis in 'xyz'])
        if any(abs(a - b) > 1e-5 for a, b in zip(node.get('translation', [0, 0, 0]), expected)):
            raise ValueError('Source and target bone translations differ')
        expected = source_direction_to_runtime([source['scale'][axis] for axis in 'xyz'])
        if any(abs(a - b) > 1e-5 for a, b in zip(node.get('scale', [1, 1, 1]), expected)):
            raise ValueError('Source and target bone scales differ')
        expected = source_rotation_to_runtime([source['rotation'][axis] for axis in 'xyzw'])
        if abs(abs(sum(a * b for a, b in zip(node.get('rotation', [0, 0, 0, 1]), expected))) - 1) > 1e-5:
            raise ValueError('Source and target bone rotations differ')
    return bones[0]['name']


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--gltf', type=Path, required=True)
    parser.add_argument('--native', type=Path, required=True)
    parser.add_argument('--asset-root', type=Path, default=Path('assets'))
    parser.add_argument('--skeleton', type=Path, required=True)
    args = parser.parse_args()
    document, blob = load_gltf(args.gltf)
    native = json.loads(args.native.read_text(encoding='utf-8-sig'))
    root_bone = validate(document, native)
    positions = [p for mesh in document['meshes'] for primitive in mesh['primitives']
        for p in accessor_values(document, blob, primitive['attributes']['POSITION'])]
    if not positions or not all(math.isfinite(value) for p in positions for value in p):
        raise ValueError('Mesh positions are empty or non-finite')
    asset_root = args.asset_root.resolve()
    mesh = args.gltf.resolve()
    skeleton = args.skeleton.resolve()
    if not mesh.is_relative_to(asset_root) or not skeleton.is_relative_to(asset_root):
        raise ValueError('Target assets must stay inside the asset root')
    if not skeleton.name.endswith('.static.skn.ron'):
        raise ValueError('Target descriptor must use the static skeleton extension')
    files = [args.gltf]
    for item in document.get('buffers', []) + document.get('images', []):
        if 'uri' not in item:
            continue
        path = (args.gltf.parent / item['uri']).resolve()
        if not path.is_relative_to(args.gltf.parent.resolve()):
            raise ValueError('Source dependency leaves the export directory')
        if not path.is_file():
            raise ValueError('Source dependency is missing')
        files.append(path)
    skeleton.parent.mkdir(parents=True, exist_ok=True)
    mesh_path = mesh.relative_to(asset_root).as_posix()
    skeleton.write_text('(\n    source: Gltf(source: ' + json.dumps(mesh_path) + ', label: "Scene0"),\n'
        '    root: ' + json.dumps(root_bone) + ',\n'
        '    source_skeleton: ' + json.dumps(native['skeletal_mesh']['skeleton_path']) + ',\n)\n', encoding='utf-8')
    print(json.dumps({'mesh': mesh_path, 'root': root_bone, 'bones': len(native['skeletal_mesh']['bones']),
        'vertices': len(positions), 'animations': len(document.get('animations', [])), 'files': len(files)}), flush=True)


def self_test():
    identity = {'translation': dict.fromkeys('xyz', 0), 'scale': dict.fromkeys('xyz', 1),
        'rotation': dict(zip('xyzw', [0, 0, 0, 1]))}
    native = {'skeletal_mesh': {'bones': [
        {'name': 'root', 'parent_index': -1, 'local_transform': deepcopy(identity)},
        {'name': 'end', 'parent_index': 0, 'local_transform': deepcopy(identity)}]}}
    source = native['skeletal_mesh']['bones'][1]['local_transform']
    source.update(translation=dict(zip('xyz', [100, 200, 300])), scale=dict(zip('xyz', [2, 3, 4])),
        rotation=dict(zip('xyzw', [math.sqrt(0.5), 0, 0, math.sqrt(0.5)])))
    document = {'skins': [{'joints': [1, 2]}], 'nodes': [{'children': [1]},
        {'name': 'root', 'children': [2]}, {'name': 'end', 'translation': [1, 3, 2],
        'scale': [2, 4, 3], 'rotation': [-math.sqrt(0.5), 0, 0, math.sqrt(0.5)]}]}
    assert validate(document, native) == 'root'
    for index, field, value in [(2, 'name', 'wrong'), (2, 'translation', [1, 2, 3]),
        (2, 'scale', [2, 3, 4]), (2, 'rotation', [0, 0, 0, 1]), (1, 'children', []),
        (2, 'children', [1])]:
        changed = deepcopy(document)
        changed['nodes'][index][field] = value
        try:
            validate(changed, native)
        except ValueError:
            continue
        raise AssertionError(f'Accepted invalid bone {field}')
    print(json.dumps({'self_test': 'passed', 'rejected': 6}), flush=True)


if __name__ == '__main__':
    self_test() if sys.argv[1:] == ['--self-test'] else main()
