import argparse
import json
import re
import sys
from copy import deepcopy
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'animgraph_authoring'))
from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE
from ue_animation_asset_resolver import object_property, source_object_path
from ue_exported_text import parse_number


def compile_definition(actor, mesh, component_source):
    component_path = source_object_path(object_property({'properties': actor['blueprint_cdo_properties']}, 'Mesh'))
    component = next(o for o in actor['blueprint_cdo_subobjects'] if o['object_path'] == component_path)
    if source_object_path(object_property(component, 'SkeletalMeshAsset')) != mesh['asset_path']:
        raise ValueError('Animation component mesh differs')
    roots = [b for b in mesh['skeletal_mesh']['bones'] if b['parent_index'] == -1]
    if len(roots) != 1:
        raise ValueError('Animation component requires one source root bone')
    tick_option = object_property(component, 'VisibilityBasedAnimTickOption')
    if tick_option not in ['OnlyTickMontagesWhenNotRendered', 'AlwaysTickPose', 'AlwaysTickPoseAndRefreshBones']:
        raise ValueError('Unsupported visibility animation tick option')
    if object_property(component, 'bUseScreenRenderStateForUpdate') != 'False':
        raise ValueError('Screen-only render history is not supported')
    windows = re.findall(r'GetWorld\(\)->TimeSeconds - ([0-9.]+)f', component_source)
    if len(windows) != 1:
        raise ValueError('Recent render window must have one source definition')
    return dict(source_mesh=mesh['asset_path'], root_bone=roots[0]['name'],
        teleport_distance_threshold=parse_number(object_property(component, 'TeleportDistanceThreshold')) * SOURCE_LENGTH_TO_RUNTIME_SCALE,
        visibility_tick_option=tick_option, recently_rendered_seconds=float(windows[0]))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--actor', type=Path, required=True)
    parser.add_argument('--mesh', type=Path, required=True)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--component-source', type=Path, required=True)
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    sources = [json.loads(p.read_text(encoding='utf-8-sig')) for p in [args.actor, args.mesh]]
    before = deepcopy(sources)
    component_source = args.component_source.read_text(encoding='utf-8-sig')
    result = compile_definition(*sources, component_source)
    if args.self_test:
        changed = deepcopy(sources)
        component_path = source_object_path(object_property({'properties': changed[0]['blueprint_cdo_properties']}, 'Mesh'))
        component = next(o for o in changed[0]['blueprint_cdo_subobjects'] if o['object_path'] == component_path)
        next(p for p in component['properties'] if p['name'] == 'TeleportDistanceThreshold')['exported_text'] = '125.0'
        assert compile_definition(*changed, component_source)['teleport_distance_threshold'] == 1.25
        changed[1]['asset_path'] = '/Fixture/DifferentMesh.DifferentMesh'
        try:
            compile_definition(*changed, component_source)
        except ValueError:
            pass
        else:
            raise AssertionError('A different component mesh was accepted')
        for name, value in [('VisibilityBasedAnimTickOption', 'Unsupported'), ('bUseScreenRenderStateForUpdate', 'True')]:
            changed = deepcopy(sources)
            component = next(o for o in changed[0]['blueprint_cdo_subobjects'] if o['object_path'] == component_path)
            next(p for p in component['properties'] if p['name'] == name)['exported_text'] = value
            try:
                compile_definition(*changed, component_source)
            except ValueError:
                pass
            else:
                raise AssertionError('Unsupported render policy was accepted')
        try:
            compile_definition(*sources, '')
        except ValueError:
            pass
        else:
            raise AssertionError('Missing recent render source was accepted')
        assert sources == before
        print(json.dumps(dict(self_test='PASS', raw_sources_unchanged=True)), flush=True)
        return
    assert sources == before
    if args.output is None or args.output.resolve() in [args.actor.resolve(), args.mesh.resolve()]:
        raise ValueError('Target output must be separate from source records')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(result), flush=True)


if __name__ == '__main__':
    main()
