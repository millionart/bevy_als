# 转场夹具根时钟

现行 `AnimationGraph::query` 将根 `TimeUpdate` 保存到查询上下文，缓存预热读取同一时钟。两个直接调用内部求值的测试此前只设置节点经过时间。其默认 IO 无根时钟，稳定返回 `TimeUpdateFailed`。

fresh UE Grounded 别名源表的222帧重放与 D207 封存表字节一致。两个测试使用实际根查询的零增量合同后，原有权重、姿态及归一化断言与全模块65项测试通过。

```sh
cargo test --profile play --lib --jobs 2 -- interruptible_fsm::tests::
```
