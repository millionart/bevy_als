import importlib.util
from pathlib import Path
import sys
import unittest


directory = Path(__file__).resolve().parents[1] / 'import'
sys.path.insert(0, str(directory))
spec = importlib.util.spec_from_file_location('import_world_solver_iterations', directory / 'import_world_solver_iterations.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class WorldSolverIterationsTest(unittest.TestCase):
    def test_defaults_overrides_and_unsupported_body_policy(self):
        settings = {'asset_properties': [{'name': 'SolverOptions', 'fields': [
            {'name': name + 'Iterations', 'exported_text': value} for name, value in
            [('Position', '8'), ('Velocity', '2'), ('Projection', '1')]]}]}
        variables = {'p.Chaos.Solver.Iterations.' + name: '-1' for name in ('Position', 'Velocity', 'Projection')}
        physics = {'physics_asset': {'constraint_console_variables': variables, 'bodies': [
            {'bone_name': 'pelvis', 'default_instance_properties': [{'name': 'bOverrideIterationCounts', 'exported_text': 'False'}]}]}}
        character = {'blueprint_cdo_subobjects': [{'object_path': '/Fixture:CharacterMesh0', 'properties': [
            {'name': 'BodyInstance', 'fields': [{'name': 'bOverrideIterationCounts', 'exported_text': 'False'}]}]}]}
        self.assertEqual(module.source_iterations(settings, physics, character), [8, 2, 1])
        variables['p.Chaos.Solver.Iterations.Velocity'] = '4'
        self.assertEqual(module.source_iterations(settings, physics, character), [8, 4, 1])
        physics['physics_asset']['bodies'][0]['default_instance_properties'][0]['exported_text'] = 'True'
        with self.assertRaises(ValueError):
            module.source_iterations(settings, physics, character)


if __name__ == '__main__':
    unittest.main()
