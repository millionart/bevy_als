# 动画资产键回归

七个原生序列与安装描述符逐一核对。五个夹具的 `ALSRUNTIME` 路径在压缩、raw FK 和 raw 曲线数据的逐字查找中均不存在。`identity.json` 记录描述符、实际键和原始数据身份。

同一 test target 中，旧键的五项均失败。改用描述符的 `ALSRuntime` 后，原数值断言及三个测试模块共十一项通过。源动画、数据和运行时调用未发生改动。

```sh
cargo test --profile play --lib --jobs 2 -- source_compression::tests:: raw_curve_sampling::tests:: source_pose::tests::
```
