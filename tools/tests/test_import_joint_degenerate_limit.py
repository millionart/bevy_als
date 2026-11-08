import importlib.util
from pathlib import Path
import unittest


spec = importlib.util.spec_from_file_location(
    'import_joint_degenerate_limit', Path(__file__).resolve().parents[1] / 'import' / 'import_joint_degenerate_limit.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class JointDegenerateLimitTest(unittest.TestCase):
    def test_source_value_and_nonfinite_rejection(self):
        source = {'physics_asset': {'constraint_console_variables': {'p.Chaos.Joint.DegenerateRotationLimit': '-0.998'}}}
        self.assertEqual(module.source_limit(source), -0.998)
        source['physics_asset']['constraint_console_variables']['p.Chaos.Joint.DegenerateRotationLimit'] = 'nan'
        with self.assertRaises(ValueError):
            module.source_limit(source)


if __name__ == '__main__':
    unittest.main()
