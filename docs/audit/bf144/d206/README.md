# D206 RichCurve 的原生浮点求值顺序

UE `CurveEvaluation.cpp:205–229` 对 Linear 段直接 Lerp，对 Cubic 段以 `tangent * span * (1/3)` 构造控制点，再用 `CurveEvaluation.h:26–35` 的三层 Lerp 求值。`AlphaBlend.cpp:314–323` 用混合值的精确相等判定完成。

fresh Grounded 编译表确认 T9 和 T19 都使用 0.5 秒 Custom 混合，生成图的三个键与 `CF_Als_StanceChange` 原生键逐值一致。旧算序在 0.4998400509 秒完成，原生在 0.4998450577 秒完成。仅恢复其中一个步骤仍有偏差。

站蹲 210、stride 162、地面动力 606 个原生样点逐值匹配。原运动样本在 gait 1.4583510160446167 的 friction 恢复为 4.541648864746094。运行时共享同一个 Bezier 核。地面动力目标配置只新增 36 个插值模式，原数值字段保持一致。旧序列化配置缺省为 Cubic，Rust 代码构造 `HermiteKey` 时须提供 `interpolation`。

15 个已安装 AnimBP 根及递归依赖均完成全量重新转换，374 个生成文件字节一致。

```sh
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SK_Als -BlendProfileProbe docs/audit/bf144/d206/curves.request.json -Output docs/audit/bf144/d206/curves.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/SK_Als -BlendProfileProbe docs/audit/bf144/d206/dynamics.request.json -Output docs/audit/bf144/d206/dynamics.native.json
pwsh -NoProfile -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/AnimationInstances/AB_Als_Grounded -StateMachinePlayers -Output docs/audit/bf144/d206/grounded.native.json
python tools/import/generate_grounded_dynamics_curves.py \
  --native docs/audit/bf144/d206/dynamics.native.json \
  --normal-settings docs/audit/bf144/d267/settings_normal.native.json \
  --responsive-settings docs/audit/bf144/d267/settings_responsive.native.json \
  --sluggish-settings docs/audit/bf144/d267/settings_sluggish.native.json \
  --baseline docs/audit/bf144/513/runtime-baseline/profile.json \
  --output assets/ALS/Data/Character/Movement/grounded_dynamics.runtime.json \
  --report docs/audit/bf144/d206/generation.json
python docs/audit/bf144/d206/replay_curve_order.py
python tools/import/test_grounded_dynamics_curves.py
python tools/animgraph_authoring/regenerate_installed_corpus.py --content-root "$ALS_CONTENT_ROOT" --output target/d206-curve-order-corpus
```
