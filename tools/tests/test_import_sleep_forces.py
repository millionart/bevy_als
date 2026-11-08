import importlib.util
from pathlib import Path
import sys
import unittest


directory = Path(__file__).resolve().parents[1] / 'import'
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location('import_sleep_forces', directory / 'import_sleep_forces.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class SleepForcesTest(unittest.TestCase):
    def test_prediction_time_and_rate(self):
        facts = [{'text': 'FReal FakeDT = (FReal)1. / (FReal)30.;'}]
        time, rate = module.parameters(facts, {'smooth_rate': '0.3'})
        self.assertAlmostEqual(time, 1.0 / 30.0)
        self.assertAlmostEqual(rate, 0.3)
        for invalid in ([], facts + facts):
            with self.assertRaises(ValueError):
                module.parameters(invalid, {'smooth_rate': '0.3'})
        self.assertEqual(module.parameters(facts, {'smooth_rate': '2'})[1], 1.0)
        with self.assertRaises(ValueError):
            module.parameters(facts, {'smooth_rate': 'nan'})


if __name__ == '__main__':
    unittest.main()
