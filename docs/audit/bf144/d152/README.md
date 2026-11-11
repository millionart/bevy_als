# D152 Fall 地面预测的独立查询胶囊

`CapsuleComponent.h:202–210` 取较小水平轴放大半径，竖直轴放大半高。`AlsAnimationInstance.cpp:786–789,1163–1190` 用这两个 getter 构造轴对齐扫掠，命中法线须达到角色的 WalkableFloorZ。B_Als_Character 原生 CDO 给出 30 cm 半径、90 cm 半高和 0.71 法线门槛。

D11 的生产修复 `5fd357b9` 已让动画查询独立构造胶囊。fresh 原生查询在平台边缘距离 40 cm 时，30 cm 半径不命中。消融改用物理 60 cm 半径后命中，法线 Z 为 0.745364。两种半径在平台中央均扫掠 210 cm 后命中。使用安装曲线复算后，只有物理胶囊在边缘产生正的误报 alpha。Fall 的原生绑定取自 [D149 Locomotion 原始报告](../d149/locomotion.native.json.gz)。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/B_Als_Character -GeometryQueryProbe docs/audit/bf144/d152/edge.request.json -Output docs/audit/bf144/d152/edge.native.json
gzip -dc docs/audit/bf144/d152/edge.native.json.gz | shasum -a 256
python docs/audit/bf144/d152/replay_prediction_capsule.py
```

首次调用额外传入 D11 的资产请求作为 CharacterAnimationProbe，缺少该探针的 gravity/cases，进程返回 7。保留首次 watchdog 报告。本轮使用 GeometryQueryProbe 重采，资产自身的 CDO 已包含所需属性。

`loaded_fall_prediction_uses_logical_capsule_at_platform_edge` 从缩放角色的实际 Fall 节点读取命中结果与 alpha。Rust、消融恢复和 Playground Gate 待统一验证。
