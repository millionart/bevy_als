import os as _os
ALS_ROOT = _os.environ.get("ALS_PROJECT_ROOT", "")
import os
import unreal

ar = unreal.AssetRegistryHelpers.get_asset_registry()
ar.wait_for_completion()
try:
    ar.scan_paths_synchronous(["/ALS/"], True)
except Exception as e:
    unreal.log_warning("PYEXPORT5 scan exc=%s" % e)

OUT = os.path.join(os.environ.get("BEVY_ALS_ROOT", os.getcwd()), "tmp", "export", "gltf_test")
os.makedirs(OUT, exist_ok=True)

mesh = unreal.load_object(None, "/ALS/ALS/Character/SKM_Als.SKM_Als")
unreal.log_warning("PYEXPORT5 mesh=%s" % (type(mesh).__name__ if mesh else None))

at = unreal.AssetToolsHelpers.get_asset_tools()
unreal.log_warning("PYEXPORT5 export_assets_doc=%s" % at.export_assets.__doc__)

if mesh:
    try:
        at.export_assets([mesh], OUT)
        unreal.log_warning("PYEXPORT5 assettools_export done")
    except Exception as e:
        unreal.log_warning("PYEXPORT5 assettools_export EXC=%s" % e)

rolls = unreal.EditorAssetLibrary.list_assets("/ALS/ALS/Animations/Actions/Roll")
unreal.log_warning("PYEXPORT5 rolls=%s" % rolls[:8])
if rolls:
    anim_obj = unreal.load_object(None, rolls[0])
    unreal.log_warning("PYEXPORT5 anim=%s cls=%s" % (
        rolls[0], type(anim_obj).__name__ if anim_obj else None))
    if anim_obj:
        try:
            at.export_assets([anim_obj], OUT)
            unreal.log_warning("PYEXPORT5 anim_export done")
        except Exception as e:
            unreal.log_warning("PYEXPORT5 anim_export EXC=%s" % e)

hits = []
for base_dir in [os.path.join(os.environ.get("BEVY_ALS_ROOT", os.getcwd()), "tmp"), os.path.join(ALS_ROOT, "Saved")]:
    for base, dirs, files in os.walk(base_dir):
        dirs[:] = [d for d in dirs if d not in (".git",)]
        for f in files:
            if f.lower().endswith((".gltf", ".glb")):
                p = os.path.join(base, f)
                hits.append((p, os.path.getsize(p)))
for h in hits[:10]:
    unreal.log_warning("PYEXPORT5 hit=%s size=%d" % h)
unreal.log_warning("PYEXPORT5 total_hits=%d" % len(hits))

skel = unreal.load_object(None, "/ALS/ALS/Character/SK_Als.SK_Als")
if skel:
    props = [p for p in ("bone_tree", "native_parent_bone_index") if hasattr(skel, p)]
    unreal.log_warning("PYEXPORT5 skel_has=%s" % props)
    try:
        bt = skel.get_editor_property("bone_tree")
        names = [str(b.name) for b in bt][:12]
        unreal.log_warning("PYEXPORT5 bones n=%d %s" % (len(bt), names))
    except Exception as e:
        unreal.log_warning("PYEXPORT5 bone_tree exc=%s" % e)

cf = unreal.load_object(None,
    "/ALS/ALS/Data/AnimationInstance/Grounded/CF_Als_StrideBlendAmount_Walk."
    "CF_Als_StrideBlendAmount_Walk")
if cf is None:
    found = unreal.EditorAssetLibrary.list_assets(
        "/ALS/ALS/Data/AnimationInstance/Grounded", recursive=True)
    unreal.log_warning("PYEXPORT5 grounded data=%s" % found[:10])
    cf = None
    for a in found:
        o = unreal.load_object(None, a)
        if o and "StrideBlendAmount_Walk" in str(a):
            cf = o
            break
if cf:
    try:
        rc = cf.get_editor_property("float_curves")
        keys = rc.keys
        pts = [(k.input_value, k.output_value) for k in keys]
        unreal.log_warning("PYEXPORT5 curve_pts=%s" % pts[:8])
    except Exception as e:
        unreal.log_warning("PYEXPORT5 curve exc=%s" % e)
