# BF144-D53 Mantle 命中组件的速度

fresh ALS ForwardTrace 使用命中的 Primitive 自己的 `GetComponentVelocity` 与阈值平方比较；UE 对非模拟 Primitive 返回自身 ComponentVelocity，模拟 Primitive 返回自身物理速度，MovementComponent 更新实际 UpdatedComponent 的速度。fresh 原生角色设置的阈值为 10 cm/s。源码片段、资源请求、reader 报告和身份在本目录；此次原生设置结果与 D43 逐字节相同，复用该 Git 跟踪文件。

目标查询直接读取命中实体上的刚体速度或组件速度，不再从父刚体继承子组件速度；未提供组件速度时按零处理。已写父子速度相反、相等、缺省、阈值边界以及加载角色实际 Mantle 对照。测试、消融与动画 Gate 待统一验证。此项只覆盖非模拟的子组件身份，尚未独立核验焊接或其他模拟子组件的对应关系。

旧清单所引 `target/bf144/16/initial/` 已不存在，其原始报告缺失。
