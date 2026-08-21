import os as _os
ALS_ROOT = _os.environ.get("ALS_PROJECT_ROOT", "")
# 探明 Interchange glTF 导出 API 面 + 单资产试导出（SKM_Als）
import os
import unreal

im = unreal.InterchangeManager.get_interchange_manager_scripted()
unreal.log_warning("PYEXPORT im_methods=%s" % [m for m in dir(im) if "export" in m.lower()])
unreal.log_warning(
    "PYEXPORT class_methods=%s" % [m for m in dir(unreal.InterchangeManager) if "export" in m.lower()]
)
at = unreal.AssetToolsHelpers.get_asset_tools()
unreal.log_warning("PYEXPORT assettools=%s" % [m for m in dir(at) if "export" in m.lower()])

mesh = unreal.EditorAssetLibrary.load_asset("/ALS/ALS/Character/SKM_Als")
unreal.log_warning("PYEXPORT loaded=%s class=%s" % (mesh, type(mesh).__name__ if mesh else None))

out_dir = os.path.join(os.environ.get("BEVY_ALS_ROOT", os.getcwd()), "tmp", "export")
os.makedirs(out_dir, exist_ok=True)

for out in ["skm_als_test.gltf", "skm_als_test.glb"]:
    path = out_dir + "/" + out
    try:
        ok = im.export_asset(mesh, path)
        unreal.log_warning("PYEXPORT try %s result=%s exists=%s size=%s" % (
            out, ok, os.path.exists(path),
            os.path.getsize(path) if os.path.exists(path) else -1))
    except Exception as e:
        unreal.log_warning("PYEXPORT try %s EXC=%s" % (out, e))

# 动画序列试导出
anim = unreal.EditorAssetLibrary.load_asset(
    "/ALS/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Forward")
unreal.log_warning("PYEXPORT bs loaded=%s" % (anim is not None))
anim2 = unreal.EditorAssetLibrary.load_asset(
    "/ALS/ALS/Animations/Actions/Roll/A_Als_Roll_Fwd")
if anim2 is None:
    # 名字不确定，列出 Roll 目录
    assets = unreal.EditorAssetLibrary.list_assets("/ALS/ALS/Animations/Actions/Roll")
    unreal.log_warning("PYEXPORT roll dir=%s" % assets)
else:
    unreal.log_warning("PYEXPORT anim loaded=%s" % anim2)
