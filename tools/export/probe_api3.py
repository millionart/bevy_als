# 探针3：注册表直查资产 + 自动导出落点定位
import os
import unreal

ar = unreal.AssetRegistryHelpers.get_asset_registry()
ar.wait_for_completion()

cls = unreal.TopLevelAssetPath("/Script/Engine", "SkeletalMesh")
assets = ar.get_assets_by_class(cls, True)
names = [str(a.package_name) for a in assets]
unreal.log_warning("PYEXPORT3 skm count=%d %s" % (len(names), names[:8]))

target = None
for n in names:
    if n.endswith("SKM_Als"):
        target = n
        break

im = unreal.InterchangeManager.get_interchange_manager_scripted()
unreal.log_warning("PYEXPORT3 export_scene_doc=%s" % im.export_scene.__doc__)

if target:
    ad = next(a for a in assets if str(a.package_name) == target)
    obj = ad.get_asset()
    unreal.log_warning("PYEXPORT3 obj=%s class=%s" % (obj, type(obj).__name__))
    ok = im.export_asset(obj, True)
    unreal.log_warning("PYEXPORT3 export=%s" % ok)

# 定位新生成的 gltf/glb（项目目录 + Saved，限深扫描）
hits = []
for root_dir in ["<als-project>", "<repo-root>/tmp"]:
    for base, dirs, files in os.walk(root_dir):
        dirs[:] = [d for d in dirs if d not in (
            ".git", "Binaries", "Intermediate", "DerivedDataCache",
            "Plugins/ALS-Refactored/Binaries", "Plugins/ALS-Refactored/Intermediate")]
        for f in files:
            if f.lower().endswith((".gltf", ".glb")):
                p = os.path.join(base, f)
                hits.append((p, os.path.getsize(p), os.path.getmtime(p)))
hits.sort(key=lambda x: -x[2])
for h in hits[:6]:
    unreal.log_warning("PYEXPORT3 hit=%s size=%d" % (h[0], h[1]))
if not hits:
    unreal.log_warning("PYEXPORT3 no gltf/glb found")
