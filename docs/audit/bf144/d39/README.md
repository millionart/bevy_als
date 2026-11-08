# BF144-D39 首次 overlap 的触碰冲量

fresh UE 源显示双方允许 overlap 时，胶囊 BeginOverlap 对模拟刚体调用 CapsuleTouched；方向混合位置与角色水平速度，速度乘 body mass 后按源最小/最大值限制。fresh 原生胶囊开启 overlap，角色参数为因子 1、按质量缩放、最小值 −1、最大值 250；请求、源摘录、原生读取、独立目标常量生成报告及哈希保存在本目录。本次未在源端重新运行动态 overlap 场景。

目标角色启用 Avian 首次接触消息；对涉及 Sensor 的首次接触，仅向动态且启用的另一刚体施加一次源式冲量，沿角色组件与对方碰撞体的世界位置计算方向。源 0.25 cm 的竖直分量和 250 kg·cm/s 的最大冲量分别生成为 0.0025 m 和 2.5 kg·m/s。已写质量、静止、非 Sensor、运动学、持续重叠及加载角色对照；测试、消融和真实入口 Gate 待统一验证阶段执行。

Avian Sensor 表示本项目现有的 overlap 配对，尚无独立目标字段表达另一组件关闭 UE GenerateOverlapEvents；短暂掠过但物理接触图未记录的 overlap 也不在本次反例内。旧清单所引 `target/bf144/12/overlap/` 已不存在，其原始报告缺失。
