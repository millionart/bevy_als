import argparse
import json
import math
import uuid
from copy import deepcopy
from pathlib import Path

from coordinate_basis import source_direction_to_runtime, source_point_to_runtime, source_rotator_to_runtime_rotation
from import_camera_component import record, vector
from import_static_skeleton import validate
from audit_animation_transform_conversion import load_gltf
from ue_animation_asset_resolver import asset_property, object_property, source_object_path, source_asset_name
from ue_animgraph_materializer import graph_path, source_asset_directory
from ue_exported_text import parse_bool, parse_list, parse_number, parse_struct


def accessor_data(document, blob, index):
    accessor = document['accessors'][index]
    view = document['bufferViews'][accessor['bufferView']]
    if accessor.get('byteOffset', 0) or 'byteStride' in view or 'sparse' in accessor or view.get('buffer', 0):
        raise ValueError('Unsupported overlay mesh accessor layout')
    start = view.get('byteOffset', 0)
    end = start + view['byteLength']
    if start < 0 or end > len(blob):
        raise ValueError('Overlay mesh accessor exceeds buffer')
    return (tuple(accessor.get(key) for key in ('componentType', 'type', 'count', 'normalized')), blob[start:end])


def compile_definition(sources, overlay):
    actor = sources['/ALS/ALS/Character/B_Als_Character.B_Als_Character']
    graph = next(g for g in actor['graphs'] if g['name'] == 'RefreshOverlayObject')
    call = next(n for n in graph['nodes'] if any(link['pin_name'] == 'Als.OverlayMode.' + overlay for p in n['pins'] for link in p['linked_to']))
    pins = {p['name']: p for p in call['pins']}
    mesh_path = pins['NewSkeletalMesh']['default_object']
    if mesh_path == 'None':
        mesh_path = pins['NewStaticMesh']['default_object']
    mesh = sources[mesh_path]
    animation = pins['NewAnimationClass']['default_object'].removesuffix('_C')
    if animation == 'None':
        animation = None
    else:
        sources[animation]
    skeleton = sources['/ALS/ALS/Character/SK_Als.SK_Als']
    socket = next(o for o in skeleton['asset_objects'] if o['class_path'].endswith('SkeletalMeshSocket') and object_property(o, 'SocketName') == pins['SocketName']['default_value'])
    materials = []
    for slot in parse_list(asset_property(mesh, 'Materials' if mesh['asset_class'].endswith('.SkeletalMesh') else 'StaticMaterials')):
        material = sources[source_object_path(parse_struct(slot)['MaterialInterface'])]
        parent = sources[source_object_path(asset_property(material, 'Parent'))]
        if (asset_property(parent, 'BlendMode'), asset_property(parent, 'ShadingModel'), asset_property(parent, 'TwoSided')) != ('BLEND_Opaque', 'MSM_DefaultLit', 'False'):
            raise ValueError('Unsupported overlay material shading')
        editor = next(o for o in parent['asset_objects'] if o['class_path'].endswith('MaterialEditorOnlyData'))
        base = record(editor['properties'], 'BaseColor')['fields']
        expression_path = source_object_path(record(base, 'Expression')['exported_text'])
        expression = next(o for o in parent['asset_objects'] if o['object_path'] == expression_path)
        if not expression['class_path'].endswith('MaterialExpressionVectorParameter'):
            raise ValueError('Overlay base color is not a vector parameter')
        parameter = object_property(expression, 'ParameterName')
        color = object_property(expression, 'DefaultValue')
        for item in parse_list(asset_property(material, 'VectorParameterValues')):
            fields = parse_struct(item)
            if parse_struct(fields['ParameterInfo'])['Name'].strip('"') == parameter:
                color = fields['ParameterValue']
        values = {'linear_rgba': vector(color, 'RGBA')}
        for source, target in [('Metallic', 'metallic'), ('Roughness', 'roughness'), ('Specular', 'reflectance')]:
            fields = record(editor['properties'], source)['fields']
            if not parse_bool(record(fields, 'UseConstant')['exported_text']):
                raise ValueError('Overlay material scalar is not constant')
            values[target] = parse_number(record(fields, 'Constant')['exported_text'])
            if source == 'Specular':
                values[target] = math.sqrt(values[target] * 0.5)
        materials.append(values)
    return {
        'source_mesh': mesh_path,
        'source_animation': animation,
        'graph': graph_path(animation, 'AnimGraph') if animation else None,
        'use_left_gun_bone': parse_bool(pins['bUseLeftGunBone']['default_value'].title()),
        'socket_name': pins['SocketName']['default_value'],
        'socket_bone': object_property(socket, 'BoneName'),
        'socket_translation': source_point_to_runtime(vector(object_property(socket, 'RelativeLocation'))),
        'socket_rotation': source_rotator_to_runtime_rotation(vector(object_property(socket, 'RelativeRotation'), ('Pitch', 'Yaw', 'Roll'))),
        'socket_scale': source_direction_to_runtime(vector(object_property(socket, 'RelativeScale'))),
        'materials': materials,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source-directory', type=Path, required=True)
    parser.add_argument('--overlay', default='Bow')
    parser.add_argument('--mesh', type=Path, required=True)
    parser.add_argument('--scene-source', type=Path, required=True)
    parser.add_argument('--skeleton', required=True)
    parser.add_argument('--asset-root', type=Path, default=Path('assets'))
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--write', action='store_true')
    mode.add_argument('--check', action='store_true')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    sources = {}
    for path in args.source_directory.glob('*.json'):
        data = json.loads(path.read_text(encoding='utf-8-sig'))
        if 'asset_path' in data:
            sources[data['asset_path']] = data
    definition = compile_definition(sources, args.overlay)
    mesh, mesh_blob = load_gltf(args.mesh)
    root = validate(mesh, sources[definition['source_mesh']])
    scene, scene_blob = load_gltf(args.scene_source)
    validate(scene, sources[definition['source_mesh']])
    if len(scene['meshes']) != len(mesh['meshes']):
        raise ValueError('Animated object mesh count differs')
    for source_mesh, scene_mesh in zip(mesh['meshes'], scene['meshes']):
        if len(source_mesh['primitives']) != len(scene_mesh['primitives']):
            raise ValueError('Animated object primitive count differs')
        for source_primitive, scene_primitive in zip(source_mesh['primitives'], scene_mesh['primitives']):
            source_accessors = source_primitive['attributes'] | {'indices': source_primitive['indices']}
            scene_accessors = scene_primitive['attributes'] | {'indices': scene_primitive['indices']}
            if source_accessors.keys() != scene_accessors.keys() or source_primitive['material'] != scene_primitive['material']:
                raise ValueError('Animated object mesh schema differs')
            for name in source_accessors:
                if accessor_data(mesh, mesh_blob, source_accessors[name]) != accessor_data(scene, scene_blob, scene_accessors[name]):
                    raise ValueError('Animated object mesh values differ: ' + name)
    if len(mesh['meshes'][0]['primitives']) != len(definition['materials']):
        raise ValueError('Overlay mesh material slots differ')
    directory = Path(source_asset_directory(definition['source_mesh'])).parent
    definition['scene'] = (directory / ('B_Als_' + args.overlay + '.animscn.ron')).as_posix()
    definition['mesh'] = args.mesh.resolve().relative_to(args.asset_root.resolve()).as_posix()
    if args.self_test:
        changed = deepcopy(sources)
        socket = next(o for o in changed['/ALS/ALS/Character/SK_Als.SK_Als']['asset_objects'] if o['class_path'].endswith('SkeletalMeshSocket') and object_property(o, 'SocketName') == definition['socket_name'])
        record(socket['properties'], 'RelativeLocation')['exported_text'] = '(X=100,Y=200,Z=300)'
        record(socket['properties'], 'RelativeRotation')['exported_text'] = '(Pitch=0,Yaw=90,Roll=0)'
        altered = compile_definition(changed, args.overlay)
        assert altered['socket_translation'] == [1.0, 3.0, 2.0]
        assert abs(altered['socket_rotation'][1] + 2 ** -0.5) < 1e-6
        assert altered['materials'] == definition['materials']
        parent = next(o for o in changed.values() if o['asset_class'].endswith('.Material'))
        editor = next(o for o in parent['asset_objects'] if o['class_path'].endswith('MaterialEditorOnlyData'))
        record(record(editor['properties'], 'Specular')['fields'], 'Constant')['exported_text'] = '0.125'
        assert all(value['reflectance'] == 0.25 for value in compile_definition(changed, args.overlay)['materials'])
        changed = deepcopy(sources)
        parent = next(o for o in changed.values() if o['asset_class'].endswith('.Material'))
        record(parent['asset_properties'], 'TwoSided')['exported_text'] = 'True'
        try:
            compile_definition(changed, args.overlay)
        except ValueError:
            pass
        else:
            raise AssertionError('Unsupported material was accepted')
    if args.write or args.check:
        output = args.asset_root / directory / (args.overlay.lower() + '.json')
        outputs = {output: json.dumps(definition, indent=2) + '\n'}
        wrapper_path = source_asset_directory(definition['source_animation']) + '.runtime.animgraph.ron'
        node = str(uuid.uuid5(uuid.NAMESPACE_URL, definition['source_animation'] + ':runtime_pose'))
        wrapper = '''(
    nodes: [
        (id: "NODE", name: NAME, inner: { "als_ext::LayeredGraphNode": (graph: GRAPH, input_aliases: [], output_aliases: [(child_output: "Result", outer_output: "pose")], capture_context: true, expose_context_curves: false, exposed_output_data: [], internalize_time: true) }, should_debug: false),
    ],
    edges_inverted: { OutputData("pose"): NodeData("NODE", "pose") },
    io_spec: (input_times: [], input_data: {}, output_time: false, output_data: { "pose": Pose }, input_order: {}, output_order: { Data("pose"): 0 }),
    default_data: {},
    editor_metadata: (node_positions: { "NODE": (-240.0, 0.0) }, input_position: (-480.0, 0.0), output_position: (0.0, 0.0), input_param_order: {}, input_time_order: {}, output_data_order: {}, output_pose_order: {}),
)
'''.replace('NODE', node).replace('NAME', json.dumps(source_asset_name(definition['source_animation']))).replace('GRAPH', json.dumps(definition['graph']))
        outputs[args.asset_root / wrapper_path] = wrapper
        scene_path = args.scene_source.resolve().relative_to(args.asset_root.resolve()).as_posix()
        outputs[args.asset_root / definition['scene']] = '(\n    source: ' + json.dumps(scene_path + '#Scene0') + ',\n    animation_graph: ' + json.dumps(wrapper_path) + ',\n    skeleton: ' + json.dumps(args.skeleton) + ',\n)\n'
        for path, value in outputs.items():
            if args.check:
                if not path.is_file() or path.read_text(encoding='utf-8') != value:
                    raise ValueError('Overlay target differs: ' + path.as_posix())
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(value, encoding='utf-8')
    print(json.dumps({'overlay': args.overlay, 'root': root, 'materials': definition['materials'], 'self_test': args.self_test}), flush=True)


if __name__ == '__main__':
    main()
