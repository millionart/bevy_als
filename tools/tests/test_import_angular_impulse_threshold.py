import importlib.util
from pathlib import Path
import sys
import unittest


directory = Path(__file__).resolve().parents[1] / 'import'
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location('import_angular_impulse_threshold', directory / 'import_angular_impulse_threshold.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class AngularImpulseThresholdTest(unittest.TestCase):
    def test_source_threshold_and_units(self):
        self.assertAlmostEqual(module.threshold([{'text': '#define UE_SMALL_NUMBER (1.e-8f)'}]), 1e-12, places=20)
        with self.assertRaises(ValueError):
            module.threshold([{'text': ''}])


if __name__ == '__main__':
    unittest.main()
