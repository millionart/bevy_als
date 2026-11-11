# D100b4b2a3 胶囊平行边与圆柱接触

源在两端已有端帽点时结束。面距离相等且首条候选边的两端都在外侧、边距离也相等时，源进入平行投影分支并直接结束。有投影落入闭区间 `[0,1]` 时，按 P0→P1 夹取边点，用未夹取投影求法线。两个投影都在边外时，按源参数规则选择一端，用实际夹取点求法线。仅保留平方距离严格小于 `(radius + cull)²` 且大于法线容差平方的点。

其余情况在 `abs(axis · faceNormal) < 0.34` 时补圆柱点。源使用 `(axis × faceNormal) × axis` 求径向，令其朝向三角面，再按三条边平面检查候选点，保留源逐点类型、顺序及面投影深度。Rock 1053 和 Crate_Broken 175/176 的第二点符合该径向支持点，距旧读取器端点重建的独立计算分别在 `5.3e-8 cm` 和 `2.5e-9 cm` 内。原始读数见 a2 的 `triangles.retry.native.json.gz`。

`prepare_parallel_cases.py request` 从 a2 的 Stairs01 面 252 和原生胶囊端点构造请求，绕源 Z 轴对齐同一条踏面边。现成读取器输出 `parallel.native.json.gz`，按两个投影都在线段内、一个在外、两个都在起点外的顺序生成 2、2、1 个 `EdgeEdge` 点。`prepare_parallel_cases.py fixture` 以请求中的绕轴旋转和位移重建胶囊空间三角，生成 focused 对照用的 `parallel-cases.json`。读取命令与 a2 相同，输入改为 `parallel.request.json`。编译、测试、消融和 Playground 待统一 Gate。
