# BF144-D37 站立动态基座的下压力

fresh UE 源显示角色移动后对有效 blocking floor 的模拟刚体，在 ImpactPoint 施加重力乘移动组件 Mass 和 StandingDownwardForceScale；fresh 原生角色分别为开启、100 kg 和 1。Avian 源显示点力按世界物理步积分并包含力矩。请求、源摘录、原生读取和哈希保存在本目录，本次没有源端动态平台逐帧实测。

已扩展现有 `tools/import/import_movement_constants.py`，从原生 CDO 生成三个独立目标常量，生成结果及参数保存在 `import.report.json`；原始资产未修改。目标在角色运动后重新读取地面接触点，仅对动态且启用的刚体施加点力，不乘角色时间倍率。重力使用项目世界重力和角色重力倍率，不将独立 Tnua 附加加速度当作 UE 的 GetGravityZ。

已写导入器属性变化及无效数据自测、受力/质量/力矩/静态对照和加载角色世界时钟对照；测试、消融及真实入口 Gate 待统一验证阶段执行。旧清单所引 `target/bf144/12/impact_downward/` 已不存在，其原始报告缺失。
