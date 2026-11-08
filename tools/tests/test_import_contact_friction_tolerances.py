import importlib.util
from pathlib import Path
import sys
import unittest


directory = Path(__file__).resolve().parents[1] / 'import'
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location('import_contact_friction_tolerances', directory / 'import_contact_friction_tolerances.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ContactFrictionTolerancesTest(unittest.TestCase):
    def test_mass_and_position_impulse_units(self):
        facts = [{'text': '#define UE_SMALL_NUMBER (1.e-8f)\n#define UE_KINDA_SMALL_NUMBER (1.e-4f)'}]
        mass, impulse = module.tolerances(facts)
        self.assertAlmostEqual(mass, 1e-8)
        self.assertAlmostEqual(impulse, 1e-6)
        for invalid in ([], facts + facts):
            with self.assertRaises(ValueError):
                module.tolerances(invalid)


if __name__ == '__main__':
    unittest.main()
