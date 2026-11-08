# BF144-D33 基座组件速度优先级

fresh UE 源显示仅 movable 基座提供继承速度；优先取组件物理所有者速度，非模拟组件取 ComponentVelocity，模拟组件取 body velocity，零值才逐级回退。fresh 原生角色默认继承轴均开启，请求、源摘录、原生读取与哈希保存在本目录；本次原生读取未驱动自定义组件速度，速度选择来自源代码。

目标共享支持速度查询现在对运动学基座优先使用已有 AlsComponentVelocity，零值回退 LinearVelocity；动态基座使用 LinearVelocity，静态基座为零。已写动力系统反例，覆盖独立/复合碰撞体、组件优先、零值回退、动态和静态对照。UE 的独立 actor/root 速度回退在本项目没有对应输入，未在本反例内推断或新增状态。

旧清单引用的 `target/bf144/11/jump_and_permissions/` 已不存在，其原始报告缺失；新测试、消融和真实入口 Gate 待统一验证阶段运行。
