# BF144-D72 初始化极小刚体质量

fresh UE `GetFinalScaled`、Chaos shape创建和 `BodyUtils` 证明初始capsule按截断后的半径与圆柱长度构造后再计算质量。仓库reader分别读取六种新建scale与从unit重缩到0.01的live控制；原生请求、watchdog报告、完整压缩输出、源码片段和身份均保留。两份输出以 `python3 tools/import/export_scaled_body_mass_probe.py --source docs/audit/bf144/d72/initial.native.json.gz --output docs/audit/bf144/d72/initial.runtime.json` 和将 `initial` 替换为 `live` 的命令转换目标单位、坐标及质量。

目标仅在新建/重建rig的uniform capsule触发初始化尺寸下限时，复用现有按实际shape合成质量、质心及惯量的路径；记录rig是否经历过live scale，以保留后续profile更新与重建的语义。未触发下限的uniform路径和live路径沿用既有计算。新测试覆盖新建0.01/0.2/1/2、profile重应用、unit→0.01 live及重建，针对原生pelvis、spine_03、foot和root；live spine_03质心另有已证差异，留D73处理，不把单shape pelvis结论推广到多shape。

测试、生命周期状态消融和真实入口均待统一执行；四个极小body的源自身数值分解偏差未用理想值改写。
