import argparse
import json
from copy import deepcopy
from pathlib import Path

from import_overlay_object import compile_definition, accessor_data
from import_static_skeleton import validate
from audit_animation_transform_conversion import load_gltf
from ue_animation_asset_resolver import object_property, source_object_path
from ue_animgraph_materializer import graph_path, source_asset_directory
from ue_exported_text import parse_list, parse_struct, parse_quoted


def compile_actor(sources, asset_root):
    actor = sources['/ALS/ALS/Character/B_Als_Character.B_Als_Character']
    classes = object_property({'properties': actor['blueprint_cdo_properties']}, 'OverlayAnimationInstanceClasses')
    attachment = next(g for g in actor['graphs'] if g['name'] == 'RefreshOverlayObject')
    attached = {link['pin_name'].removeprefix('Als.OverlayMode.') for node in attachment['nodes']
        if any(pin['name'] == 'NewSkeletalMesh' for pin in node['pins'])
        for pin in node['pins'] if pin['name'] == 'execute' for link in pin['linked_to']}
    result = {}
    for entry in parse_list(classes):
        key, value = parse_list(entry)
        tag = parse_quoted(parse_struct(key)['TagName'])
        if not tag.startswith('Als.OverlayMode.'):
            raise ValueError('Unexpected overlay tag')
        name = tag.removeprefix('Als.OverlayMode.')
        source = source_object_path(parse_quoted(value)).removesuffix('_C')
        layer = next(g for g in sources[source]['graphs'] if g['name'] == 'Overlay')
        blend = next(p for p in layer['properties'] if p['name'] == 'BlendOptions')
        fields = {p['name']: p['exported_text'] for p in blend['fields']}
        if fields['BlendInProfile'] != 'None' or fields['BlendOutProfile'] != 'None':
            raise ValueError('Linked layer blend profiles are unsupported: ' + source)
        definition = {'source_class': source, 'graph': graph_path(source, 'AnimGraph'),
            'blend_in': float(fields['BlendInTime']), 'blend_out': float(fields['BlendOutTime']), 'object': None}
        if name in attached:
            obj = compile_definition(sources, name)
            mesh_source = sources[obj['source_mesh']]
            skeletal = mesh_source['asset_class'] == '/Script/Engine.SkeletalMesh'
            path = asset_root / (source_asset_directory(obj['source_mesh']) + '.gltf')
            mesh, blob = load_gltf(path)
            if skeletal:
                validate(mesh, mesh_source)
            primitives = [primitive for item in mesh['meshes'] for primitive in item['primitives']]
            if [primitive['material'] for primitive in primitives] != list(range(len(obj['materials']))):
                raise ValueError('Object material slot order differs: ' + name)
            for primitive in primitives:
                accessor_data(mesh, blob, primitive['attributes']['POSITION'])
            obj['mesh'] = path.relative_to(asset_root).as_posix()
            obj['kind'] = 'Skeletal' if skeletal else 'Static'
            obj['scene'] = Path(obj['mesh']).with_name('B_Als_' + name + '.animscn.ron').as_posix() if obj['source_animation'] else None
            if obj['scene'] and not (asset_root / obj['scene']).is_file():
                raise ValueError('Object animation scene is missing: ' + name)
            definition['object'] = obj
        result[tag] = definition
    if {'Als.OverlayMode.' + name for name in attached} - result.keys():
        raise ValueError('Overlay actor coverage differs')
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source-directory', type=Path, nargs='+', required=True)
    parser.add_argument('--asset-root', type=Path, default=Path('assets'))
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--write', action='store_true')
    mode.add_argument('--check', action='store_true')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    sources = {value['asset_path']: value for directory in args.source_directory for path in directory.glob('*.ue.json')
        if 'asset_path' in (value := json.loads(path.read_text(encoding='utf-8-sig')))}
    result = compile_actor(sources, args.asset_root)
    if args.self_test:
        assert result['Als.OverlayMode.PistolOneHanded']['object'] == result['Als.OverlayMode.PistolTwoHanded']['object']
        assert result['Als.OverlayMode.Bow']['object']['scene'] == 'ALS/OverlayObjects/Bow/B_Als_Bow.animscn.ron'
        assert result['Als.OverlayMode.Barrel']['object']['materials'][0]['linear_rgba'] == [0.93, 0.29, 1.0, 1.0]
        assert result['Als.OverlayMode.Binoculars']['object']['materials'][0]['linear_rgba'] == [0.015, 0.015, 0.015, 1.0]
        changed = deepcopy(sources)
        actor = changed['/ALS/ALS/Character/B_Als_Character.B_Als_Character']
        field = next(p for p in actor['blueprint_cdo_properties'] if p['name'] == 'OverlayAnimationInstanceClasses')
        field['exported_text'] = field['exported_text'].replace('AB_Als_Masculine', 'AB_Als_Rifle')
        assert compile_actor(changed, args.asset_root)['Als.OverlayMode.Masculine']['graph'].endswith('/AB_Als_Rifle/AnimGraph.animgraph.ron')
        assert result['Als.OverlayMode.Default']['blend_in'] == result['Als.OverlayMode.Rifle']['blend_out'] == 0.2
        layer = next(g for g in changed[result['Als.OverlayMode.Rifle']['source_class']]['graphs'] if g['name'] == 'Overlay')
        blend = next(p for p in layer['properties'] if p['name'] == 'BlendOptions')
        next(p for p in blend['fields'] if p['name'] == 'BlendInTime')['exported_text'] = '0.6'
        assert compile_actor(changed, args.asset_root)['Als.OverlayMode.Masculine']['blend_in'] == 0.6
        next(p for p in blend['fields'] if p['name'] == 'BlendInProfile')['exported_text'] = 'Profile'
        try:
            compile_actor(changed, args.asset_root)
        except ValueError as error:
            assert 'blend profiles' in str(error)
        else:
            raise AssertionError('Unsupported blend profile was accepted')
        bow = json.loads((args.asset_root / 'ALS/OverlayObjects/Bow/bow.json').read_text())
        assert {key: result['Als.OverlayMode.Bow']['object'][key] for key in bow} == bow
    path = args.asset_root / 'ALS/Character/overlay_actor.runtime.json'
    tag_path = args.asset_root / 'ALS/Data/als_gameplay_tags.json'
    tags = json.loads(tag_path.read_text(encoding='utf-8'))
    registered = {tag['tag_name']: tag for tag in tags}
    if len(registered) != len(tags):
        raise ValueError('Gameplay tag table contains duplicate names')
    for name in result:
        registered.setdefault(name, {'tag_name': name, 'description': 'ALS source gameplay tag'})
    tag_text = json.dumps([registered[name] for name in sorted(registered)], indent=2) + '\n'
    text = json.dumps(result, indent=2) + '\n'
    if args.write:
        path.write_text(text, encoding='utf-8')
        tag_path.write_text(tag_text, encoding='utf-8')
    elif not path.is_file() or path.read_text(encoding='utf-8') != text:
        raise ValueError('Overlay actor definition differs')
    elif tag_path.read_text(encoding='utf-8') != tag_text:
        raise ValueError('Overlay gameplay tag registration differs')
    print(json.dumps({'classes': len(result), 'object_modes': sum(value['object'] is not None for value in result.values()), 'self_test': args.self_test}))


if __name__ == '__main__':
    main()
