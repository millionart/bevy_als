# D122 GetUp Montage 曲线

两套原生 Montage 各采样 15 个片段时点。精确键的源求值重放覆盖 330 个曲线值。序列提供的 PoseCrouching 与 PoseGrounded 进入同一个 PostLocomotion 槽。

```sh
python docs/audit/bf144/d122/replay_get_up_curves.py --output target/d122/recheck --content-root "$ALS_CONTENT_ROOT" --source-cpp "$ALS_ACTIONS_CPP"
```

`curve-comparison.json` 记录原生样本、舍入消融及当前目标采样算术的复算误差。
