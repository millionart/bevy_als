import argparse
import json
import math
from copy import deepcopy
from pathlib import Path

from import_moving_actors import box_definition, transform, vector
from coordinate_basis import quaternion_multiply, source_rotator_to_runtime_rotation
from ue_animgraph_materializer import source_asset_directory
from ue_animation_asset_resolver import object_property, source_object_path
from ue_exported_text import parse_bool, parse_struct


def rotation_rate(values):
    if not all(math.isfinite(value) for value in values):
        raise ValueError('Rotation rate must be finite')
    pitch, yaw, roll = map(math.radians, values)
    return [-yaw, pitch, roll]


def validate_rotation_graph(blueprint):
    graph = next(graph for graph in blueprint['graphs'] if graph['name'] == 'RefreshMeshRotation')
    nodes = {node['object_path']: node for node in graph['nodes']}
    def member(node, key='FunctionReference'):
        value = next((prop['exported_text'] for prop in node['properties'] if prop['name'] == key), '()')
        return parse_struct(value).get('MemberName', '').strip('"')
    def input_node(node, name):
        links = next(pin for pin in node['pins'] if pin['name'] == name)['linked_to']
        if len(links) != 1:
            raise ValueError('Rotation input must have one source')
        return nodes[links[0]['node_object_path']]
    setter = next(node for node in nodes.values() if member(node) == 'K2_SetRelativeRotation')
    product = input_node(setter, 'NewRotation')
    clock = input_node(product, 'B')
    if member(input_node(setter, 'self'), 'VariableReference') != 'StaticMesh' \
        or member(product) != 'Multiply_RotatorFloat' or member(input_node(product, 'A'), 'VariableReference') != 'RotationSpeed' \
        or member(clock) != 'Add_DoubleDouble' or {member(input_node(clock, name)) for name in ['A', 'B']} != {'GetServerWorldTimeSeconds', 'GetFirstPlayerPingSeconds'}:
        raise ValueError('Rotation graph no longer uses the source world-time rule')


def compile_definitions(world, blueprint, mesh):
    validate_rotation_graph(blueprint)
    objects = {item['object_path']: item for item in world['asset_objects']}
    collider = box_definition(mesh)
    actors = []
    for actor in world['asset_objects']:
        if actor['class_path'] != blueprint['asset_path'] + '_C':
            continue
        def component(name):
            reference = object_property(actor, name)
            return objects[source_object_path(reference) if "'" in reference else actor['object_path'] + '.' + reference]
        root, primitive = component('RootComponent'), component('StaticMesh')
        if object_property(root, 'AttachParent') != 'None' or object_property(root, 'Mobility') != 'Static':
            raise ValueError('Rotating actor requires an unattached static anchor')
        if object_property(primitive, 'AttachParent') != root['object_path'].rsplit('.', 1)[1] or object_property(primitive, 'Mobility') != 'Movable':
            raise ValueError('Rotating primitive attachment or mobility differs')
        if source_object_path(object_property(primitive, 'StaticMesh')) != mesh['asset_path']:
            raise ValueError('Rotating source mesh differs')
        body = next(p for p in primitive['properties'] if p['name'] == 'BodyInstance')
        physics = {p['name']: p['exported_text'] for p in body['fields']}
        if physics['CollisionEnabled'] != 'QueryAndPhysics' or physics['CollisionProfileName'] != 'BlockAllDynamic' or parse_bool(physics['bSimulatePhysics']):
            raise ValueError('Rotating primitive physics differs')
        actors.append(dict(name=actor['object_path'].rsplit('.', 1)[1], actor=transform(root), primitive=transform(primitive),
            mesh_scene=source_asset_directory(mesh['asset_path']) + '.gltf#Scene0',
            mesh_base=mesh['asset_path'].rsplit('.', 1)[1], **collider,
            rotation_rate_yzx_radians=rotation_rate(vector(object_property(actor, 'RotationSpeed'), ['Pitch', 'Yaw', 'Roll']))))
    if not actors:
        raise ValueError('No source rotating actors were found')
    actors.sort(key=lambda actor: actor['name'])
    return dict(format='als_rotating_actors_v1', source_world=world['asset_path'], source_actor=blueprint['asset_path'], actors=actors)


def self_test(sources):
    for angles in [(90, 0, 0), (0, 90, 0), (0, 0, 90), (23, 67, -41), (-19, 123, 16)]:
        for time in [0.0, 0.13, 1.0, 3.27]:
            rate = rotation_rate(angles)
            rotations = []
            for axis, value in zip([1, 2, 0], rate):
                q = [0.0, 0.0, 0.0, math.cos(value*time/2)]
                q[axis] = math.sin(value*time/2)
                rotations.append(q)
            actual = quaternion_multiply(quaternion_multiply(rotations[0], rotations[1]), rotations[2])
            expected = source_rotator_to_runtime_rotation([value*time for value in angles])
            assert min(math.dist(actual, expected), math.dist(actual, [-v for v in expected])) < 1e-12
    baseline = compile_definitions(*sources)
    changed = deepcopy(sources)
    primitive = next(o for o in changed[0]['asset_objects'] if o['object_path'].endswith(baseline['actors'][0]['name'] + '.StaticMesh'))
    for name, value in [('Mobility', 'Static'), ('AttachParent', 'None'), ('StaticMesh', "None")]:
        prop = next(p for p in primitive['properties'] if p['name'] == name)
        saved = prop['exported_text']
        prop['exported_text'] = value
        try:
            compile_definitions(*changed)
        except (ValueError, KeyError):
            pass
        else:
            raise AssertionError('Invalid rotating component was accepted: ' + name)
        prop['exported_text'] = saved
    assert compile_definitions(*changed) == baseline
    physics = next(prop for prop in primitive['properties'] if prop['name'] == 'BodyInstance')['fields']
    simulate = next(prop for prop in physics if prop['name'] == 'bSimulatePhysics')
    simulate['exported_text'] = 'True'
    try:
        compile_definitions(*changed)
    except ValueError:
        pass
    else:
        raise AssertionError('A simulated rotating primitive was accepted')
    for values in [(0.0, float('inf'), 0.0), (float('nan'), 0.0, 0.0)]:
        try:
            rotation_rate(values)
        except ValueError:
            pass
        else:
            raise AssertionError('A non-finite rotation rate was accepted')
    changed = deepcopy(sources)
    graph = next(graph for graph in changed[1]['graphs'] if graph['name'] == 'RefreshMeshRotation')
    setter = next(node for node in graph['nodes'] if node['title'].startswith('Set Relative Rotation'))
    next(pin for pin in setter['pins'] if pin['name'] == 'NewRotation')['linked_to'] = []
    try:
        compile_definitions(*changed)
    except ValueError:
        pass
    else:
        raise AssertionError('A disconnected source rotation graph was accepted')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--world', type=Path, required=True)
    parser.add_argument('--blueprint', type=Path, required=True)
    parser.add_argument('--mesh', type=Path, required=True)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    paths = [args.world, args.blueprint, args.mesh]
    sources = [json.loads(path.read_text(encoding='utf-8-sig')) for path in paths]
    before = deepcopy(sources)
    result = compile_definitions(*sources)
    if args.self_test:
        self_test(sources)
    else:
        if args.output is None or args.output.resolve() in [path.resolve() for path in paths]:
            raise ValueError('Output must be separate from source records')
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    assert sources == before
    print(json.dumps(dict(actors=len(result['actors']), raw_sources_unchanged=True, self_test=args.self_test)), flush=True)


if __name__ == '__main__':
    main()
