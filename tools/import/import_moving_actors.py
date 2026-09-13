import argparse
import json
import math
import sys
from copy import deepcopy
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'animgraph_authoring'))
from coordinate_basis import source_point_to_runtime, source_direction_to_runtime, source_rotator_to_runtime_rotation, quaternion_multiply
from gen_ragdoll import quat_rotate
from ue_animation_asset_resolver import asset_property, object_property, source_object_path
from ue_exported_text import parse_struct, parse_list, parse_number, parse_bool


def vector(text, axes='XYZ'):
    fields = parse_struct(text)
    return [parse_number(fields.get(axis, '0')) for axis in axes]


def transform(component):
    return dict(translation=source_point_to_runtime(vector(object_property(component, 'RelativeLocation'))),
        rotation=source_rotator_to_runtime_rotation(vector(object_property(component, 'RelativeRotation'), ['Pitch', 'Yaw', 'Roll'])),
        scale=source_direction_to_runtime(vector(object_property(component, 'RelativeScale3D'))))


def point(frame, value):
    rotated = quat_rotate(frame['rotation'], [a*b for a, b in zip(frame['scale'], value)])
    return [a+b for a, b in zip(frame['translation'], rotated)]


def compose(parent, local):
    return dict(translation=point(parent, local['translation']),
        rotation=quaternion_multiply(parent['rotation'], local['rotation']),
        scale=[a*b for a, b in zip(parent['scale'], local['scale'])])


def box_definition(mesh):
    mesh_objects = {item['object_path']: item for item in mesh['asset_objects']}
    setup = mesh_objects[source_object_path(asset_property(mesh, 'BodySetup'))]
    geometry = parse_struct(object_property(setup, 'AggGeom'))
    if set(geometry) != {'BoxElems'}:
        raise ValueError('Moving mesh requires a single source box collider')
    boxes = parse_list(geometry['BoxElems'])
    if len(boxes) != 1:
        raise ValueError('Moving mesh requires a single source box collider')
    box = parse_struct(boxes[0])
    center = source_point_to_runtime(vector(box.get('Center', '()')))
    rotation = source_rotator_to_runtime_rotation(vector(box.get('Rotation', '()'), ['Pitch', 'Yaw', 'Roll']))
    half_extents = [v*0.5 for v in source_point_to_runtime([parse_number(box[axis]) for axis in 'XYZ'])]
    return dict(collider_center=center, collider_rotation=rotation, half_extents=half_extents)


def compile_definitions(world, blueprint, mesh):
    objects = {item['object_path']: item for item in world['asset_objects']}
    collider = box_definition(mesh)
    actors = []
    for actor in world['asset_objects']:
        if actor['class_path'] != blueprint['asset_path'] + '_C':
            continue
        def component(name):
            reference = object_property(actor, name)
            path = source_object_path(reference) if "'" in reference else actor['object_path'] + '.' + reference
            return objects[path]
        root, spline, primitive = (component(name) for name in ['RootComponent', 'MovementSpline', 'StaticMesh'])
        root_name = root['object_path'].rsplit('.', 1)[1]
        if object_property(root, 'AttachParent') != 'None' or object_property(root, 'Mobility') != 'Static':
            raise ValueError('Moving actor root must be an unattached static anchor')
        for child, mobility in [(spline, 'Static'), (primitive, 'Movable')]:
            if object_property(child, 'AttachParent') != root_name or object_property(child, 'Mobility') != mobility:
                raise ValueError('Moving component attachment or mobility differs')
        if source_object_path(object_property(primitive, 'StaticMesh')) != mesh['asset_path']:
            raise ValueError('Moving mesh source differs')
        body = next(p for p in primitive['properties'] if p['name'] == 'BodyInstance')
        physics = {p['name']: p['exported_text'] for p in body['fields']}
        if physics['CollisionEnabled'] != 'QueryAndPhysics' or physics['CollisionProfileName'] != 'BlockAll' or parse_bool(physics['bSimulatePhysics']):
            raise ValueError('Moving component physics configuration differs')
        if parse_bool(object_property(spline, 'bClosedLoop')):
            raise ValueError('Moving spline must be open')
        curves = parse_struct(object_property(spline, 'SplineCurves'))
        points = [parse_struct(value) for value in parse_list(parse_struct(curves['Position'])['Points'])]
        if len(points) != 2 or [parse_number(p.get('InVal', '0')) for p in points] != [0.0, 1.0]:
            raise ValueError('Moving spline requires two ordered source points')
        positions = [vector(p.get('OutVal', '()')) for p in points]
        span = [b-a for a, b in zip(*positions)]
        for p in points:
            if p.get('InterpMode') != 'CIM_Linear' and any(
                math.dist(vector(p.get(name, '()')), span) > 1e-5 for name in ['ArriveTangent', 'LeaveTangent']):
                raise ValueError('Moving spline is not a straight source segment')
        duration = parse_number(object_property(actor, 'MovementDuration'))
        if duration <= 0:
            raise ValueError('Moving duration must be positive')
        actor_frame = transform(root)
        spline_frame = compose(actor_frame, transform(spline))
        actors.append(dict(name=actor['object_path'].rsplit('.', 1)[1], actor=actor_frame, primitive=transform(primitive),
            mesh_scene='models/playground/' + mesh['asset_path'].rsplit('.', 1)[1] + '.gltf#Scene0',
            mesh_base=mesh['asset_path'].rsplit('.', 1)[1], **collider,
            start=point(spline_frame, source_point_to_runtime(positions[0])), end=point(spline_frame, source_point_to_runtime(positions[1])), duration=duration))
    if not actors:
        raise ValueError('No source moving actors were found')
    actors.sort(key=lambda value: value['name'])
    return dict(format='als_moving_actors_v1', source_world=world['asset_path'], source_actor=blueprint['asset_path'], actors=actors)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--world', required=True, type=Path)
    parser.add_argument('--blueprint', required=True, type=Path)
    parser.add_argument('--mesh', required=True, type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    paths = [args.world, args.blueprint, args.mesh]
    sources = [json.loads(path.read_text(encoding='utf-8-sig')) for path in paths]
    before = deepcopy(sources)
    result = compile_definitions(*sources)
    if args.self_test:
        assert len(result['actors']) == 2
        horizontal = next(value for value in result['actors'] if value['duration'] == 1.75)
        assert horizontal['start'] == [19.0, 0.0, -28.75] and horizontal['end'] == [12.5, 0.0, -28.75]
        changed = deepcopy(sources)
        root = next(value for value in changed[0]['asset_objects'] if value['object_path'].endswith(horizontal['name'] + '.DefaultSceneRoot'))
        for name, value in [('RelativeLocation', '(X=100,Y=200,Z=300)'), ('RelativeRotation', '(Pitch=0,Yaw=90,Roll=0)'), ('RelativeScale3D', '(X=2,Y=3,Z=4)')]:
            next(p for p in root['properties'] if p['name'] == name)['exported_text'] = value
        moved = next(value for value in compile_definitions(*changed)['actors'] if value['name'] == horizontal['name'])
        assert math.dist(moved['start'], [1, 3, 2]) < 1e-6 and math.dist(moved['end'], [1, 3, -11]) < 1e-6
        next(p for p in root['properties'] if p['name'] == 'Mobility')['exported_text'] = 'Movable'
        try:
            compile_definitions(*changed)
        except ValueError:
            pass
        else:
            raise AssertionError('An unsupported moving anchor was accepted')
        for invalid in ['physics', 'spline']:
            changed = deepcopy(sources)
            actor_path = next(o['object_path'] for o in changed[0]['asset_objects'] if o['object_path'].endswith('.' + horizontal['name']))
            if invalid == 'physics':
                primitive = next(o for o in changed[0]['asset_objects'] if o['object_path'] == actor_path + '.StaticMesh')
                fields = next(p for p in primitive['properties'] if p['name'] == 'BodyInstance')['fields']
                next(p for p in fields if p['name'] == 'bSimulatePhysics')['exported_text'] = 'True'
            else:
                spline = next(o for o in changed[0]['asset_objects'] if o['object_path'] == actor_path + '.MovementSpline')
                next(p for p in spline['properties'] if p['name'] == 'bClosedLoop')['exported_text'] = 'True'
            try:
                compile_definitions(*changed)
            except ValueError:
                pass
            else:
                raise AssertionError('Unsupported source configuration was accepted')
        assert sources == before
        print(json.dumps(dict(self_test='PASS', actors=len(result['actors']), raw_sources_unchanged=True)), flush=True)
        return
    if args.output is None or args.output.resolve() in [path.resolve() for path in paths]:
        parser.error('A separate target output is required')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(dict(result='PASS', actors=len(result['actors']))), flush=True)


if __name__ == '__main__':
    main()
