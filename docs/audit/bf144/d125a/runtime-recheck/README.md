# 胶囊扫掠复验

本地 UE 的 `GJK.h` 以胶囊端点的 simplex 权重重建正时接触点，`GeometryQueries.cpp` 先将端点变换到箱体空间。`UnrealMathVectorCommon.h.inl` 的叉积旋转在此测试中保留转轴上的分量。原 Glam 展开在 f32 四元数提升为 double 后改变该分量，`walking` 样本接触高度相差 0.16920078 m。

原始和相同 f32 投影输入分别复采原生数据，两次接触高度一致。复用 Control Rig 已验证的源旋转公式，只替换扫掠起点的逆旋转，恢复 14 组原生样本。其余旋转替换通过消融移除。定向测试同时覆盖四组穿透、反向扫掠、场景过滤和角色地面预测。

正式 Playground 的固定 60 Hz 上坡起跳消融及恢复均通过 18 组。默认可变帧步首次运行通过前五组，第六组进入 Jump 后保持 Grounded 并超时。该条件继续由 U07 追踪。

三个 Playground 报告以无损 gzip 保存。复验时将相应 `report.json.gz` 解压为同目录的 `report.json`，运行 `tools/validate_animation_visual_evidence.py --report <report.json> --min-screenshots 0 --check --functional-only`。每个目录的 `archives.identity.json` 记录解压后的字节数和 SHA-256。
