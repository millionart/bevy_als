import argparse
import gzip
import json
from pathlib import Path

from import_mesh_pawn_response import field


def source_iterations(settings, physics, character):
    options = field(settings['asset_properties'], 'SolverOptions')['fields']
    variables = physics['physics_asset']['constraint_console_variables']
    for body in physics['physics_asset']['bodies']:
        if field(body['default_instance_properties'], 'bOverrideIterationCounts')['exported_text'] != 'False':
            raise ValueError(f"Unsupported body iteration override: {body['bone_name']}")
    meshes = [item for item in character['blueprint_cdo_subobjects'] if item['object_path'].endswith(':CharacterMesh0')]
    if len(meshes) != 1:
        raise ValueError('Expected one character mesh')
    mesh_body = field(meshes[0]['properties'], 'BodyInstance')
    if field(mesh_body['fields'], 'bOverrideIterationCounts')['exported_text'] != 'False':
        raise ValueError('Unsupported character mesh iteration override')
    result = []
    for name in ('Position', 'Velocity', 'Projection'):
        value = int(field(options, name + 'Iterations')['exported_text'])
        override = int(variables['p.Chaos.Solver.Iterations.' + name])
        effective = override if override >= 0 else value
        if effective < 0:
            raise ValueError(f'Invalid {name} iteration count')
        result.append(effective)
    return result


def read(path):
    opener = gzip.open if path.suffix == '.gz' else open
    with opener(path, 'rt', encoding='utf-8-sig') as source:
        return json.load(source)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--settings', type=Path, required=True)
    parser.add_argument('--physics', type=Path, required=True)
    parser.add_argument('--character', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in [path.resolve() for path in (args.settings, args.physics, args.character)]:
        raise ValueError('Target output must differ from source')
    result = source_iterations(read(args.settings), read(args.physics), read(args.character))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text('[' + ', '.join(str(value) for value in result) + ']\n', encoding='utf-8')


if __name__ == '__main__':
    main()
