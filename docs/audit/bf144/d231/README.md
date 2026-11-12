# D231 源节点末尾归一化

BlendSpace 和 MultiWay 在普通累积后按有效输入数执行源 API 与节点末尾阶段。单输入执行一次，多输入执行两次，逐骨 BlendSpace API 自身也执行归一化。目标两次归一化在 double 中连续执行，根运动属性维持原合成路径。Local 和 Mesh ApplyAdditive 在有效 alpha 下各补齐节点末尾阶段。

当前 WalkRun 的20个单输入、120个双输入共11060骨，按原生 API 输出重放节点阶段全部精确。移除外层阶段改变121个 f32 投影。实际 Head 的6组输入共474骨全部精确，alpha 为0的对照保持原值。003:cc 的 `thumb_03_l` 重现既有 f32 反例。

```sh
python docs/audit/bf144/d231/replay_normalization_stages.py
```

ARM Rust fixture 覆盖 float 输入提升后连续归一化的可观测差异。合成输入及中间姿态的源厘米/double 保留由 D232 验收。Cargo 与可见 Playground 统一后置。
