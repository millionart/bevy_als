import argparse
import json
import math
import re
from pathlib import Path

from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE
from import_mesh_pawn_response import field
from import_movement_constants import NUMBER, literal


def parameters(facts, settings, native):
    source = '\n'.join(item['text'] for item in facts)
    options = field(settings['asset_properties'], 'SolverOptions')['exported_text']
    position_total = int(literal(options, rf'\bPositionIterations\s*=\s*{NUMBER}', 'position iterations'))
    velocity_total = int(literal(options, rf'\bVelocityIterations\s*=\s*{NUMBER}', 'velocity iterations'))
    position_default = int(literal(source, rf'NumPositionFrictionIterations\({NUMBER}\)', 'position friction iterations'))
    velocity_default = int(literal(source, rf'NumVelocityFrictionIterations\({NUMBER}\)', 'velocity friction iterations'))
    values = native['console_variables']
    position_override = int(values['p.Chaos.Solver.Collision.PositionFrictionIterations'])
    velocity_override = int(values['p.Chaos.Solver.Collision.VelocityFrictionIterations'])
    position_count = min(position_total, position_override if position_override >= 0 else position_default)
    velocity_count = min(velocity_total, velocity_override if velocity_override >= 0 else velocity_default)
    normal_override = float(values['p.Chaos.Collision.Stiffness'])
    normal_stiffness = normal_override if normal_override >= 0 else 1.0
    position_stiffness = float(values['p.Chaos.PBDCollisionSolver.Position.StaticFriction.Stiffness']) * normal_stiffness
    velocity_stiffness = float(values['p.Chaos.PBDCollisionSolver.Velocity.StaticFriction.Stiffness']) * normal_stiffness
    if not all(math.isfinite(value) for value in (normal_stiffness, position_stiffness, velocity_stiffness)):
        raise ValueError('Invalid source friction stiffness')
    if velocity_count != 1 or velocity_stiffness != 1.0 or values['p.Chaos.Solver.Use1DFriction'] != 'false':
        raise ValueError('Unsupported source velocity friction mode')
    if float(native['head_material']['BaseFrictionImpulse']) != 0.0 or float(values['p.CollisionBaseFrictionImpulse']) > 0.0:
        raise ValueError('Unsupported source minimum friction impulse')
    if native['head_material']['SoftCollisionMode'] != 'None':
        raise ValueError('Unsupported source soft collision mode')
    geometry_squared = literal(source, rf'#define\s+UE_KINDA_SMALL_NUMBER\s+\({NUMBER}f?\)', 'tangent normalization')
    velocity_squared = geometry_squared * SOURCE_LENGTH_TO_RUNTIME_SCALE ** 2
    return position_count, position_stiffness, velocity_squared, geometry_squared


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--facts', type=Path, required=True)
    parser.add_argument('--settings', type=Path, required=True)
    parser.add_argument('--native', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in (args.facts.resolve(), args.settings.resolve(), args.native.resolve()):
        raise ValueError('Target output must differ from source')
    values = parameters(json.loads(args.facts.read_text()), json.loads(args.settings.read_text(encoding='utf-8-sig')),
                        json.loads(args.native.read_text()))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(f'({values[0]}_u32, {values[1]:.9g}_f32, {values[2]:.9g}_f32, {values[3]:.9g}_f32)\n', encoding='utf-8')


if __name__ == '__main__':
    main()
