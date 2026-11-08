# BF144-D94a 位置摩擦数值合同

fresh PA的head材质动态摩擦0.7、静摩擦字段0，源合并后静摩擦使用两者最大值0.7；源位置摩擦刚度为0.5、速度刚度为1，位置阶段位于世界八轮求解的后四轮。fresh原生输出与D65a受跟踪记录逐字节相同，相关CVar、reader请求/报告和源函数片段已保存。

独立计算接受两侧逆质量、逆惯量、固定力臂、位置/旋转增量、两条切向基与法向位置冲量，按源分轴有效质量、线性位置误差和静/动摩擦锥更新累计冲量及双方修正。执行 `python3 tools/import/import_contact_friction_tolerances.py --facts docs/audit/bf144/d94a/source-facts.json --output src/avian_ext/contact_friction_tolerances.rs` 可重建有效质量和位置冲量阈值；后者从kg·cm转为kg·m。

已写原生head惯量的单轴闭式对照、双body双轴、静摩擦锥裁剪与释放、零摩擦和有效质量边界。代码尚未接入物理调度；D94b负责位置/速度阶段交接，D96负责非零历史锚点。测试、消融及Playground待统一Gate。
