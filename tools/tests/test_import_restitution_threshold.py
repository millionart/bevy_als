import importlib.util
from pathlib import Path
import sys
import unittest


directory = Path(__file__).resolve().parents[1] / 'import'
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location('import_restitution_threshold', directory / 'import_restitution_threshold.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class RestitutionThresholdTest(unittest.TestCase):
    def test_default_override_and_unit_conversion(self):
        facts = [{'text': 'static constexpr int32 DefaultRestitutionThreshold = 1000;'}]
        self.assertEqual(module.threshold(facts, {'collision_restitution_threshold_override': '-1'}), 10.0)
        self.assertEqual(module.threshold(facts, {'collision_restitution_threshold_override': '200'}), 2.0)
        self.assertEqual(module.threshold(facts, {'collision_restitution_threshold_override': '0'}), 0.0)
        with self.assertRaises(ValueError):
            module.threshold([], {'collision_restitution_threshold_override': '-1'})
        with self.assertRaises(ValueError):
            module.threshold(facts, {'collision_restitution_threshold_override': 'nan'})


if __name__ == '__main__':
    unittest.main()
