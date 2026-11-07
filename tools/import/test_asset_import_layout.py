import contextlib
import io
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import gen_curves
import gen_ron
import audit_als_animation_events


class AssetImportLayoutTests(unittest.TestCase):
    def test_notify_audit_classifies_exported_assets_by_source_class(self):
        with tempfile.TemporaryDirectory() as directory:
            content = Path(directory)
            objects = {
                'ALSCamera/SKM_Test': 'SkeletalMesh',
                'ALS/OverlayObjects/Bow/A_Test': 'AnimSequence',
            }
            manifest = content / 'exports.md'
            manifest.write_text('\n'.join(
                f'| date | `/ALS/{name}.{Path(name).name}` | glTF | `assets/{name}.gltf` |'
                for name in [*objects, 'ALS/Animations/A_Missing']
            ))
            for name in objects:
                path = content / (name + '.uasset')
                path.parent.mkdir(parents=True, exist_ok=True)
                path.touch()
            def load(path):
                name = Path(path).relative_to(content).with_suffix('').as_posix()
                return SimpleNamespace(asset_class=objects[name], exports=[SimpleNamespace(object_name=Path(name).name)])
            with patch.object(audit_als_animation_events, 'UAsset', side_effect=load), \
                 patch.object(audit_als_animation_events, 'PropReader'), \
                 patch.object(audit_als_animation_events, 'export_class', side_effect=lambda asset, index: asset.asset_class), \
                 patch.object(audit_als_animation_events, 'event_rows', side_effect=lambda asset, reader, index, source: source):
                rows, failures = audit_als_animation_events.audit(content, manifest)
            self.assertEqual([row['asset_class'] for row in rows], ['AnimSequence'])
            self.assertEqual(failures, [{'source_object': '/ALS/ALS/Animations/A_Missing.A_Missing', 'error': 'missing_exported_animation'}])

    def test_curve_tables_stay_with_their_source_module(self):
        with tempfile.TemporaryDirectory() as directory:
            assets = Path(directory) / 'assets'
            curve = assets / 'ALSCamera/Data/CF_Test.json'
            curve.parent.mkdir(parents=True)
            curve.write_text(json.dumps({'sampling': {'mode': 'uniform'}, 'keys': [{'t': 0, 'v': 1}]}))
            other = assets / 'actors.json'
            other.write_text('[]')
            rich = assets / 'ALSCamera/Data/CF_Test.rich.json'
            rich.write_text('{"keys": [{"time": 0, "value": 1}]}')
            with patch.object(gen_curves, 'CURVES', assets), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(gen_curves.main(), 0)
            self.assertEqual(curve.with_suffix('.curve.ron').read_text(), '(mode: Linear, keys: [(t: 0.0, v: 1.0)])\n')
            self.assertFalse(other.with_suffix('.curve.ron').exists())
            self.assertFalse(rich.with_suffix('.curve.ron').exists())

    def test_clip_import_includes_overlay_objects_and_skips_static_meshes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            assets = root / 'assets'
            clips = ['ALS/Animations/Base/A_Test', 'ALS/OverlayObjects/Bow/A_Test_Bow']
            for clip in clips:
                path = assets / (clip + '.gltf')
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(json.dumps({'scenes': [{'nodes': [0]}], 'nodes': [{}], 'animations': [{
                    'name': path.stem + '_0', 'samplers': [{}], 'channels': [{'sampler': 0, 'target': {'node': 0}}],
                }]}))
            mesh = assets / 'ALS/Character/SM_Static.gltf'
            mesh.parent.mkdir(parents=True, exist_ok=True)
            mesh.write_text('{"scenes": [{"nodes": [0]}], "nodes": [{}]}')
            with patch.multiple(gen_ron, REPO=root, ASSETS=assets, MODELS=assets, ANIMS=assets/'ALS', SKELS=assets/'ALS/Character',
                                SKELETON_SOURCES={'SKM_Als': (clips[0] + '.gltf', 'Scene0')}), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(gen_ron.main(), 0)
            self.assertIn('ALS/Character/SKM_Als.skn.ron', (assets / (clips[0] + '.anim.ron')).read_text())
            self.assertIn('ALS/OverlayObjects/Bow/SKM_Als_Bow.skn.ron', (assets / (clips[1] + '.anim.ron')).read_text())
            self.assertFalse(mesh.with_suffix('.anim.ron').exists())
            self.assertFalse((assets / 'skeletons').exists())
            runtime = assets / 'ALSRuntime/Animations/Base/A_Test.gltf'
            runtime.parent.mkdir(parents=True)
            runtime.write_text((assets / (clips[0] + '.gltf')).read_text())
            with patch.multiple(gen_ron, REPO=root, ASSETS=assets, MODELS=assets, ANIMS=assets/'ALS', SKELS=assets/'ALS/Character',
                                SKELETON_SOURCES={'SKM_Als': (clips[0] + '.gltf', 'Scene0')}), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(gen_ron.main(), 0)
            self.assertIn('ALSRuntime/Animations/Base/A_Test.gltf', (assets / (clips[0] + '.anim.ron')).read_text())
            self.assertIn(clips[1] + '.gltf', (assets / (clips[1] + '.anim.ron')).read_text())


if __name__ == '__main__':
    unittest.main()
