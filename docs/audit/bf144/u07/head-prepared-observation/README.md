# Head 的实际 BlendSpace 采样观察

UE `FAnimNode_BlendSpaceEvaluator::UpdateAssetPlayer` 把 Head 的归一时间交给原 BlendSpace。`UBlendSpace::GetAnimationPose` 在普通样本路径直接调用压缩序列，再按实际权重组合。D224d 已记录 Look 的 baked mesh additive 数据与独立数值重放。

目标 prepared 路径也直接采样，旧观察依赖的 ClipNode 与 ReferenceMeshDifference 缓存不会写入。观察改为读取实际 BlendSpace 绝对时间 pin，从目标包按独立 Head yaw 和 pitch 重算预期样本及组合，并核对实际 Head 输出和回调资格。非 prepared 路径仍沿现有实际节点读取。图和资产未改。

正式 `head` 完成俯视、平视、仰视、速度方向输入和释放、第一人称无缩放时钟，以及返回第三人称。`source.native.json.gz` 是独立的原生84帧实际角色输入历史，含 ±60° pitch、±170°和180° yaw；`source-boundaries.json` 提取七处输入段末实际时钟和活跃样本。原生与正式用例的完整输入轨迹各自保留。
