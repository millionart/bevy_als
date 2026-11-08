import importlib.util
from pathlib import Path
import sys
import unittest


directory = Path(__file__).resolve().parents[1] / 'import'
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location('import_sleep_velocity_tolerance', directory / 'import_sleep_velocity_tolerance.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class SleepVelocityToleranceTest(unittest.TestCase):
    def test_source_tolerance_and_units(self):
        facts = [{'text': '#define UE_KINDA_SMALL_NUMBER (1.e-4f)'}]
        linear, angular = module.tolerances(facts)
        self.assertAlmostEqual(linear, 1e-6)
        self.assertAlmostEqual(angular, 1e-4)
        for invalid in ([], facts + facts):
            with self.assertRaises(ValueError):
                module.tolerances(invalid)


if __name__ == '__main__':
    unittest.main()
