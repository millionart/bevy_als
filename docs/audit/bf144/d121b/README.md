# D121b 同步与片段时长

`timing-comparison.json` 对照 26 个当前原生序列的时长、RateScale 和同步标记。六位小数对照在蹲走 Left Backward 的第 28 帧仍使用旧标记区间。该帧的位置在两个 Right 标记时点之间。

`corpus-diff.json` 与 `full_diff.patch` 覆盖安装清单里的全部生成文件。转换使用 UE 的英语 culture，保留来源资产和节点名称。默认系统语言下的失败记录和同一资产的英语源报告可核对 locale 对照。

```sh
python tools/animgraph_authoring/regenerate_installed_corpus.py --self-test
python docs/audit/bf144/d121b/verify_timing.py "$ALS_CONTENT_ROOT"
python tools/animgraph_authoring/regenerate_installed_corpus.py --content-root "$ALS_CONTENT_ROOT" --output target/d121b/recheck
```

`mantle-native-inputs.zip` 的条目和解压后字节哈希见 `mantle-native-identities.json`。六个原生动作蒙太奇使用 D121a 目录内的 `mantle-*.native.json`，运行 Mantle 生成器时按原生资产对象名提供相应的 `.ue.json` 文件。
