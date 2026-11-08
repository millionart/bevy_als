# BF144-D73 live胶囊与多shape质量

fresh `UpdateBodyScale` 使用源shape原尺寸、当前scale和 `FCollisionShape` 的0.0001cm半轴/半径下限，创建时的0.1cm圆柱长度下限不适用于live更新。原生reader取得unit→1/0.75/1.5/0.01的实际body质量、质心和主惯量。`python3 tools/import/export_live_capsule_probe.py --source docs/audit/bf144/d73/live.native.json.gz --output docs/audit/bf144/d73/live.runtime.json` 将原生质量数据转为目标单位，并依据记录的UE公式与PA原始半径/长度计算预期胶囊几何；几何不是reader直接读取的原生shape。

额外0.001 scale的reader返回1并报告UE `CombineWorldSpace` 零总质量ensure，故其输出保留为失败证据而不作为验证样本；原stdout包含本地路径，仅保留哈希和不含路径的原始错误行。原清单缓存反例也已缺失。

目标按现有rig生命周期选择初始或live胶囊构造；live触发半径/半轴截断时按更新后shape复算质量、质心、惯量。已写全capsule四档几何、pelvis与spine等body原生质量/质心对照及源公式边界fixture；测试、消融和真实物理轨迹/Playground待统一执行。
