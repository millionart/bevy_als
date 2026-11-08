import importlib.util
from pathlib import Path
import sys
import unittest


directory = Path(__file__).resolve().parents[1] / 'import'
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location('export_live_capsule_probe', directory / 'export_live_capsule_probe.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class LiveCapsuleProbeTest(unittest.TestCase):
    def test_live_capsule_minimums_in_source_units(self):
        body = {'bone_name': 'pelvis', 'mass_kg': 0.001, 'principal_inertia_kg_cm2': {'x': 1, 'y': 2, 'z': 3},
                'mass_space_local': {'rotation': {'x': 0, 'y': 0, 'z': 0, 'w': 1},
                                     'translation': {'x': 0, 'y': 0, 'z': 0}}}
        source = {'asset_path': '/Fixture/Physics', 'physics_asset': {'bodies': [
            {'bone_name': 'pelvis', 'shapes': [{'index': 0, 'kind': 'capsule', 'radius': 14, 'length': 7}]}]},
            'physics_body_probe': {'rescale_from_unit': True, 'samples': [
                {'scale': {'x': scale, 'y': scale, 'z': scale}, 'bodies': [body]} for scale in (0.01, 0.001)]}}
        first, second = module.convert(source)['samples']
        self.assertAlmostEqual(first['capsules'][0]['radius_m'], 0.0014)
        self.assertAlmostEqual(first['capsules'][0]['half_length_m'], 0.00035)
        self.assertAlmostEqual(second['capsules'][0]['radius_m'], 0.000175)
        self.assertAlmostEqual(second['capsules'][0]['half_length_m'], 0.000001)


if __name__ == '__main__':
    unittest.main()
