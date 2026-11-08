import importlib.util
from pathlib import Path
import sys
import unittest


directory = Path(__file__).resolve().parents[1] / 'import'
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location('export_body_inertia_probe', directory / 'export_body_inertia_probe.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class BodyInertiaProbeTest(unittest.TestCase):
    def test_basis_and_inertia_units(self):
        source = {'asset_path': '/Fixture/Physics', 'physics_body_probe': {'samples': [{
            'scale': {'x': 1, 'y': 2, 'z': 3}, 'bodies': [{
                'bone_name': 'head', 'principal_inertia_kg_cm2': {'x': 1, 'y': 2, 'z': 3},
                'mass_space_local': {'rotation': {'x': 0.5, 'y': 0.5, 'z': 0.5, 'w': 0.5}},
            }],
        }]}}
        sample = module.convert(source)['samples'][0]
        self.assertEqual(sample['scale'], [1, 3, 2])
        for actual, expected in zip(sample['bodies'][0]['principal_kg_m2'], [0.0001, 0.0003, 0.0002]):
            self.assertAlmostEqual(actual, expected, places=12)
        self.assertEqual(sample['bodies'][0]['rotation_xyzw'], [-0.5, -0.5, -0.5, 0.5])


if __name__ == '__main__':
    unittest.main()
