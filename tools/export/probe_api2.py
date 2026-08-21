import os as _os
ALS_ROOT = _os.environ.get("ALS_PROJECT_ROOT", "")
# 探针2：等待资产注册表 + 导出签名确认
import unreal

ar = unreal.AssetRegistryHelpers.get_asset_registry()
ar.wait_for_completion()
unreal.log_warning("PYEXPORT2 registry ready")

im = unreal.InterchangeManager.get_interchange_manager_scripted()
unreal.log_warning("PYEXPORT2 doc=%s" % im.export_asset.__doc__)

mesh = unreal.EditorAssetLibrary.load_asset("/ALS/ALS/Character/SKM_Als")
unreal.log_warning("PYEXPORT2 mesh=%s" % mesh)

if mesh:
    out = os.path.join(os.environ.get("BEVY_ALS_ROOT", os.getcwd()), "tmp", "export", "skm_als_test.gltf")
    # 签名含 is_automated，按 doc 提示尝试常见位置
    for args in [(True,), ()]:
        try:
            ok = im.export_asset(mesh, out, *args)
            unreal.log_warning("PYEXPORT2 result=%s args=%s" % (ok, args))
            break
        except Exception as e:
            unreal.log_warning("PYEXPORT2 fail args=%s exc=%s" % (args, e))
