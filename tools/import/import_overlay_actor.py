import argparse
import json
from copy import deepcopy
from pathlib import Path

from import_overlay_object import compile_definition, accessor_data
from import_static_skeleton import validate
from audit_animation_transform_conversion import load_gltf
from ue_animation_asset_resolver import object_property, source_object_path
from ue_animgraph_materializer import graph_path
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
        definition = {'source_class': source, 'graph': graph_path(source, 'AnimGraph', 'generated'), 'object': None}
        if name in attached:
            obj = compile_definition(sources, name)
            mesh_source = sources[obj['source_mesh']]
            basename = obj['source_mesh'].rsplit('.', 1)[-1]
            skeletal = mesh_source['asset_class'] == '/Script/Engine.SkeletalMesh'
            path = asset_root / 'models' / ('' if skeletal else 'overlay') / (basename + '.gltf')
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
            obj['scene'] = 'animated_scenes/als_' + name.lower() + '.animscn.ron' if obj['source_animation'] else None
            if obj['scene'] and not (asset_root / obj['scene']).is_file():
                raise ValueError('Object animation scene is missing: ' + name)
            definition['object'] = obj
        result[name] = definition
    if len(result) != 13 or set(attached) - result.keys():
        raise ValueError('Overlay actor coverage differs')
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source-directory', type=Path, required=True)
    parser.add_argument('--asset-root', type=Path, default=Path('assets'))
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--write', action='store_true')
    mode.add_argument('--check', action='store_true')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    sources = {value['asset_path']: value for path in args.source_directory.glob('*.ue.json')
        if 'asset_path' in (value := json.loads(path.read_text(encoding='utf-8-sig')))}
    result = compile_actor(sources, args.asset_root)
    if args.self_test:
        assert result['PistolOneHanded']['object'] == result['PistolTwoHanded']['object']
        assert result['Bow']['object']['scene'] == 'animated_scenes/als_bow.animscn.ron'
        assert result['Barrel']['object']['materials'][0]['linear_rgba'] == [0.93, 0.29, 1.0, 1.0]
        assert result['Binoculars']['object']['materials'][0]['linear_rgba'] == [0.015, 0.015, 0.015, 1.0]
        changed = deepcopy(sources)
        actor = changed['/ALS/ALS/Character/B_Als_Character.B_Als_Character']
        field = next(p for p in actor['blueprint_cdo_properties'] if p['name'] == 'OverlayAnimationInstanceClasses')
        field['exported_text'] = field['exported_text'].replace('AB_Als_Masculine', 'AB_Als_Changed')
        assert compile_actor(changed, args.asset_root)['Masculine']['graph'].endswith('/AB_Als_Changed/AnimGraph.animgraph.ron')
        bow = json.loads((args.asset_root / 'character/overlay_objects/bow.json').read_text())
        assert {key: result['Bow']['object'][key] for key in bow} == bow
    path = args.asset_root / 'character/overlay_actor.runtime.json'
    text = json.dumps(result, indent=2) + '\n'
    if args.write:
        path.write_text(text, encoding='utf-8')
    elif not path.is_file() or path.read_text(encoding='utf-8') != text:
        raise ValueError('Generated overlay actor differs')
    print(json.dumps({'classes': len(result), 'object_modes': sum(value['object'] is not None for value in result.values()), 'self_test': args.self_test}))


if __name__ == '__main__':
    main()
