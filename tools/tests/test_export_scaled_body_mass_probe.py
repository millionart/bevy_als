import importlib.util
from pathlib import Path
import sys
import unittest


directory = Path(__file__).resolve().parents[1] / 'import'
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location('export_scaled_body_mass_probe', directory / 'export_scaled_body_mass_probe.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ScaledBodyMassProbeTest(unittest.TestCase):
    def test_mass_and_center_units(self):
        source = {'asset_path': '/Fixture/Physics', 'physics_body_probe': {'rescale_from_unit': True, 'samples': [{
            'scale': {'x': 1, 'y': 1, 'z': 1}, 'bodies': [{
                'bone_name': 'pelvis', 'mass_kg': 0.001, 'principal_inertia_kg_cm2': {'x': 1, 'y': 2, 'z': 3},
                'mass_space_local': {'rotation': {'x': 0, 'y': 0, 'z': 0, 'w': 1},
                                     'translation': {'x': 100, 'y': 200, 'z': 300}},
            }],
        }]}}
        result = module.convert(source)
        self.assertTrue(result['rescale_from_unit'])
        self.assertEqual(result['samples'][0]['bodies'][0]['mass_kg'], 0.001)
        self.assertEqual(result['samples'][0]['bodies'][0]['center_m'], [1, 3, 2])


if __name__ == '__main__':
    unittest.main()
