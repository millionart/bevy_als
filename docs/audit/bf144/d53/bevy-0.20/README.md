# D53 命中组件速度复验

源 ALS 使用命中 Primitive 的 GetComponentVelocity，非模拟组件返回自己的 ComponentVelocity。父刚体的物理速度可与命中子组件的速度不同。当前 compound 矩阵明确提供命中组件速度，保留根刚体模拟、权限覆盖和相对空间检查。正式 moving_base 复用 D60 的同一生产版本报告。
