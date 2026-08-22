import os as _os
ALS_ROOT = _os.environ.get("ALS_PROJECT_ROOT", "")
import unreal

ar = unreal.AssetRegistryHelpers.get_asset_registry()

mp = unreal.Paths.get_mount_point_for_content_dir(
    os.path.join(ALS_ROOT, "Plugins", "ALS-Refactored", "Content", ""))
unreal.log_warning("PYEXPORT4 mount=%s" % mp)

unreal.log_warning("PYEXPORT4 scan_doc=%s" % ar.scan_paths_synchronous.__doc__)
try:
    ar.scan_paths_synchronous(["/ALS/"], True)
    unreal.log_warning("PYEXPORT4 scan ok")
except Exception as e:
    unreal.log_warning("PYEXPORT4 scan exc=%s" % e)

unreal.log_warning("PYEXPORT4 list_char=%s" %
                   unreal.EditorAssetLibrary.list_assets("/ALS/ALS/Character")[:6])

obj = unreal.load_object(None, "/ALS/ALS/Character/SKM_Als.SKM_Als")
unreal.log_warning("PYEXPORT4 load_object=%s class=%s" %
                   (obj, type(obj).__name__ if obj else None))

if obj:
    im = unreal.InterchangeManager.get_interchange_manager_scripted()
    ok = im.export_asset(obj, True)
    unreal.log_warning("PYEXPORT4 export=%s" % ok)
