# D121c CurveFloat 源精度

三条目标曲线的源键来自原生反射字段。独立的 UE 曲线探针提供原生键和 243 个采样。复算脚本使用同一源求值顺序比较精确键和原有六位小数文本。

```sh
python docs/audit/bf144/d121c/replay_curve_products.py --output target/d121c/recheck --content-root "$ALS_CONTENT_ROOT" --source-cpp "$ALS_ANIMATION_CPP"
python tools/animgraph_authoring/regenerate_installed_corpus.py --content-root "$ALS_CONTENT_ROOT" --output target/d121c/recheck-corpus
```

`curve-settings-diff.json` 记录 Standing 和 Air 产物的数值变化及实际 F32 变化。
