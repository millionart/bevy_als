import sys

import unreal


if len(sys.argv) != 2:
    raise RuntimeError("expected exactly one asset path")

asset_path = sys.argv[1]
asset = unreal.load_asset(asset_path)
if asset is None:
    raise RuntimeError(f"asset was not found: {asset_path}")

subsystem = unreal.get_editor_subsystem(unreal.AssetEditorSubsystem)
if not subsystem.open_editor_for_assets([asset]):
    raise RuntimeError(f"asset editor did not open: {asset_path}")
