import importlib.util
import math
from pathlib import Path
import sys
import unittest


directory = Path(__file__).resolve().parents[1] / 'import'
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location('import_contact_pushout', directory / 'import_contact_pushout.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ContactPushoutTest(unittest.TestCase):
    def test_world_override_and_unlimited(self):
        settings = {'asset_properties': [{'name': 'SolverOptions', 'exported_text': '(CollisionMaxPushOutVelocity=1000.000000)'}]}
        self.assertEqual(module.pushout(settings, {'max_pushout_override': '-1'}), 10.0)
        self.assertEqual(module.pushout(settings, {'max_pushout_override': '400'}), 4.0)
        self.assertTrue(math.isinf(module.pushout(settings, {'max_pushout_override': '0'})))
        with self.assertRaises(ValueError):
            module.pushout({'asset_properties': []}, {'max_pushout_override': '-1'})


if __name__ == '__main__':
    unittest.main()
