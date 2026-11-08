import importlib.util
from pathlib import Path
import sys
import unittest


directory = Path(__file__).resolve().parents[1] / 'import'
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location('import_physics_substeps', directory / 'import_physics_substeps.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class PhysicsSubstepsTest(unittest.TestCase):
    def test_synchronous_single_step_and_unsupported_modes(self):
        settings = {'asset_properties': [{'name': 'bSubstepping', 'exported_text': 'False'},
                                         {'name': 'bTickPhysicsAsync', 'exported_text': 'False'},
                                         {'name': 'MaxSubsteps', 'exported_text': '6'}]}
        self.assertEqual(module.source_substeps(settings), 1)
        for index in (0, 1):
            settings['asset_properties'][index]['exported_text'] = 'True'
            with self.assertRaises(ValueError):
                module.source_substeps(settings)
            settings['asset_properties'][index]['exported_text'] = 'False'


if __name__ == '__main__':
    unittest.main()
