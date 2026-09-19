import argparse
import json
import sys
from collections import Counter
from copy import deepcopy
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'animgraph_authoring'))
from ue_animation_asset_resolver import source_object_path


def channels(obj):
    value = obj.get('blocking_query_channels')
    if not isinstance(value, list) or any(not isinstance(item, str) or not item for item in value) or len(set(value)) != len(value):
        raise ValueError('Native blocking query channels are missing or invalid')
    return value


def compile_profiles(character, playground):
    properties = {prop['name']: prop['exported_text'] for prop in character['blueprint_cdo_properties']}
    roles = {}
    for role, property_name in [('capsule', 'CapsuleComponent'), ('mesh', 'Mesh')]:
        reference = source_object_path(properties[property_name])
        matches = [obj for obj in character['blueprint_cdo_subobjects'] if obj['object_path'] == reference]
        if len(matches) != 1:
            raise ValueError(f'Character {role} source is missing or ambiguous')
        roles[role] = channels(matches[0])
    actors = {}
    for obj in playground['asset_objects']:
        if obj['class_path'] != '/Script/Engine.StaticMeshComponent':
            continue
        path = obj['object_path'].partition(':PersistentLevel.')[2]
        actor, separator, _ = path.partition('.')
        if not actor or not separator:
            raise ValueError('Scene collision component has no actor identity')
        value = channels(obj)
        if actor in actors and actors[actor] != value:
            raise ValueError('Scene actor has different component query responses')
        actors[actor] = value
    if not actors:
        raise ValueError('Scene query responses are empty')
    common = list(Counter(tuple(value) for value in actors.values()).most_common(1)[0][0])
    overrides = {name: value for name, value in sorted(actors.items()) if value != common}
    return roles, common, overrides, actors


def self_test(character, playground):
    roles, common, overrides, actors = compile_profiles(character, playground)
    assert 'Visibility' not in roles['capsule'] and 'Camera' in roles['capsule']
    assert 'Visibility' not in roles['mesh'] and 'Camera' in roles['mesh']
    assert 'Visibility' in common and 'Camera' in common
    assert any('Visibility' in value and 'Camera' not in value for value in overrides.values())
    assert all(overrides.get(name, common) == value for name, value in actors.items())
    disabled = 0
    for obj in playground['asset_objects']:
        if 'blocking_query_channels' not in obj:
            continue
        body = next(prop for prop in obj['properties'] if prop['name'] == 'BodyInstance')
        enabled = next(prop['exported_text'] for prop in body['fields'] if prop['name'] == 'CollisionEnabled')
        if enabled in {'NoCollision', 'PhysicsOnly', 'ProbeOnly'}:
            assert not channels(obj)
            disabled += 1
    assert disabled > 0
    for case in ['missing', 'ambiguous']:
        changed = deepcopy(playground)
        obj = next(obj for obj in changed['asset_objects'] if obj['class_path'] == '/Script/Engine.StaticMeshComponent')
        if case == 'missing':
            del obj['blocking_query_channels']
        else:
            duplicate = deepcopy(obj)
            duplicate['blocking_query_channels'] = ['Visibility'] if not channels(obj) else []
            changed['asset_objects'].append(duplicate)
        try:
            compile_profiles(character, changed)
        except ValueError:
            pass
        else:
            raise AssertionError(f'Invalid {case} source response was accepted')
    return {'passed': True, 'actors': len(actors), 'overrides': len(overrides), 'disabled_primitives': disabled}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--character', type=Path, required=True)
    parser.add_argument('--playground', type=Path, required=True)
    parser.add_argument('--output-directory', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    character = json.loads(args.character.read_text(encoding='utf-8-sig'))
    playground = json.loads(args.playground.read_text(encoding='utf-8-sig'))
    if args.self_test:
        print(json.dumps(self_test(character, playground)), flush=True)
        return
    if args.output_directory is None:
        parser.error('An independent target output directory is required')
    if any(args.output_directory.resolve() == source.parent.resolve() for source in [args.character, args.playground]):
        raise ValueError('Target outputs must be separate from raw source records')
    roles, common, overrides, actors = compile_profiles(character, playground)
    args.output_directory.mkdir(parents=True, exist_ok=True)
    for role, value in roles.items():
        (args.output_directory / f'als_{role}.query_channels.ron').write_text(json.dumps(value) + '\n', encoding='utf-8')
    text = '(' + json.dumps(common) + ', ' + json.dumps(overrides, indent=2) + ')\n'
    (args.output_directory / 'playground.query_channels.ron').write_text(text, encoding='utf-8')
    print(json.dumps({'passed': True, 'actors': len(actors), 'overrides': len(overrides)}), flush=True)


if __name__ == '__main__':
    main()
