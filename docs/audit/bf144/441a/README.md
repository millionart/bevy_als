# BF144.441aR CurveVector 探针证据

本目录保存当前 reader 的独立复跑。原实现见提交 `ee52e16a`。清单曾引用的旧 target 报告和身份快照已缺失。

`prepare.py` 由已有 fixture 生成三轴、旧 Float 和无效 axis 请求，源资料由本地 UE 读取。 `controls.ps1` 记录基线，分别固定轴 0、移除数字类型保护，然后恢复 reader 源码并重建同一 Mac 包。探针直接运行原生读取器。 `finalize.py` 核对完整兼容性、成功及失败边界和原始 JSON 的无损压缩。

在仓库根目录设置 `ALS_UE_SOURCE_ROOT` 指向本地 UE 源码根目录。按顺序运行 `run_python.ps1 -Name prepare`、`controls.ps1` 和 `run_python.ps1 -Name finalize`。复跑应使用新的证据目录或干净的工作副本，保留已有报告。构建包可重建，长期报告、请求、恢复身份和校验结果都随 Git 管理。
