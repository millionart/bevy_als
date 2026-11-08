import importlib.util
from pathlib import Path
import sys
import unittest


directory = Path(__file__).resolve().parents[1] / 'import'
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location('import_contact_friction_settings', directory / 'import_contact_friction_settings.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ContactFrictionSettingsTest(unittest.TestCase):
    def test_source_iterations_stiffness_and_thresholds(self):
        facts = [{'text': 'NumPositionFrictionIterations(4), NumVelocityFrictionIterations(1)\n#define UE_KINDA_SMALL_NUMBER (1.e-4f)'}]
        settings = {'asset_properties': [{'name': 'SolverOptions', 'exported_text': '(PositionIterations=8,VelocityIterations=2)'}]}
        values = {'p.Chaos.Solver.Collision.PositionFrictionIterations': '-1', 'p.Chaos.Solver.Collision.VelocityFrictionIterations': '-1',
                  'p.Chaos.Collision.Stiffness': '-1', 'p.Chaos.PBDCollisionSolver.Position.StaticFriction.Stiffness': '0.5',
                  'p.Chaos.PBDCollisionSolver.Velocity.StaticFriction.Stiffness': '1', 'p.Chaos.Solver.Use1DFriction': 'false',
                  'p.CollisionBaseFrictionImpulse': '-1'}
        native = {'console_variables': values, 'head_material': {'BaseFrictionImpulse': '0', 'SoftCollisionMode': 'None'}}
        count, stiffness, speed, geometry = module.parameters(facts, settings, native)
        self.assertEqual((count, stiffness), (4, 0.5))
        self.assertAlmostEqual(speed, 1e-8)
        self.assertAlmostEqual(geometry, 1e-4)
        values['p.Chaos.Solver.Use1DFriction'] = 'true'
        with self.assertRaises(ValueError):
            module.parameters(facts, settings, native)


if __name__ == '__main__':
    unittest.main()
