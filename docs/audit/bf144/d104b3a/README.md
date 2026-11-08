# BF144-D104b3a：源三角网格边界与身份

fresh 读取 Playground 七个三角网格，源 bounds 来自物理三角网格；Cart、Fountain 和 Rock 的源最大轴 float 厘米与现有渲染 glTF 边界重算值不同，见 `target-source-bounds.json`。另有 Fountain 与 Stairs03 的非均匀、负缩放原生样本；九个样本按源缩放规则得到的 float 最大轴长度均一致。请求、原始压缩报告、watchdog、资产身份与哈希见 `native-identities.json`。

运行 `python3 tools/import/import_trimesh_contact_bounds.py --source docs/audit/bf144/d104b3a/bounds.native.json.gz --output assets/ALSExtras/Environment/Meshes/static_trimesh_bounds.runtime.json` 可重新生成目标配置。真实 Playground 网格标记携带源资产名，凸包和网格共享源边界缩放规则。已写导出身份、七个源网格、两个缩放及真实 Stairs03 标记控制；跨组复用与求解槽接入留待后续。按统一顺序，完整转换及 diff、构建、测试、消融和 Playground 验证均未执行。
