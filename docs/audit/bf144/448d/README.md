# BF144.448d 额外scene-root实际应用资格

原Playground319 after和318有效before读取实际mapped mesh、AnimationTargetId/AnimatedBy、所有匹配target与local/fresh计算world Transform。Pose的SKM_Als三通道逐值等于.b完整Root，包括44次非恒等scale；实际mapped实体确为Mesh3d/SkinnedMesh，但两个动画目标组件全部缺失，319帧匹配目标数均0，local T/Q/S始终恒等。

当前 `apply_animation_to_visible_targets` 只遍历具备目标id/owner的实体，并检查refresh_bones再写Some通道。因此query100的Pose scale0.9769455790519714没有写入mesh实际scale1；这个候选在当前应用链排除，不新计差异、不删除Pose数据，也不据此关闭其余79骨和曲线余差。

四个缺entry/伪绑定/改local scale/伪consumer反例拒。删除观察字段后原场景check缺字段失败，其余完整旧历史保持；三源码原始bytes/SHA恢复、同target六库重编后，完整新旧数组（包括实际entity资格、local/world）及check报告精确恢复，两functional Gate通过。396现行身份/六库/原main及生产/图/loader未变，原始参数历史无损gzip并核解压bytes/SHA，全部长期证据Git管理。

复验使用已有输入：`pwsh docs/audit/bf144/448d/run_python.ps1 -Name check -Case restored`。新真实运行用新Label。截图0、性能暂停，GPU周边不验收；无新增编号，271未修，完整Root余差继续.c库存所列直接producer。
