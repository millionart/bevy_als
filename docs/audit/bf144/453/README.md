# BF144.453：完整 Head 平移／缩放投影

复用 `.451b` 的全部1,272四域原请求、native输出、当前公开核输出。fresh源full-alpha按厘米F64累计T和乘S，目标按runtime米制F32组合。两端各100,488个骨行的T/S数值逐值精确；只计原79骨，不把其余Pose字段、None表示或额外场景节点算作本项等价。

`source` 在源厘米double组合后投runtime F32；`early-unit` 先转runtime米制double再组合；`current` 子输入先投F32并按实际F32组合。全部四域的结果如下。

| 输入域 | source/current位置差 | source/current缩放差 | 单独提前单位的中间位置变化 |
|---|---:|---:|---:|
| current/current | 0 | 0 | 0 |
| source/current | 2564 | 1487 | 36 |
| current/source | 48 | 0 | 0 |
| source/source | 403 | 0 | 0 |

额外的“子输入F32、用F64组合后再投F32”控制，在全部100,488个T/S骨行上与current相同，故删除。prototype的原源码、完整输出和删除证明保留，剩余三域逐值保持；源单位变化36行与子输入变化存在交互，不能相加。SS的403位置余差由这一子输入投影路径完整复现；首frame1的ik_hand_gun z：源投影0.0045656925067305565，当前0.004565692041069269。没有改变叶导出系数或把它归D275；扩展既有D232。

删提前单位控制后分析因无信号失败；省去当前位置的子输入F32投影后，首`000:sc` spine_03实际端点合同失败。原失败与首数值保存在`negative-controls.json`，日志不是唯一证据。源码以字节恢复，同target六库重新编译后，完整输出、分析和表格字节精确。

复验：`run_replay.ps1 -Label <new-label> -Build`，再`run_python.ps1 -Name analyze -Case <new-label>`；`prepare`从Git原记录重建输入，首次显式.gz走普通JSON的错误已由实际完整重跑纠正。设置`ALS_UE_SOURCE_ROOT`后运行`facts`/`identity`核对source。完整消融的保存文件已有Git原件，`controls.ps1`拒绝覆盖既有保存基线；可直接使用`mutate.py <control>`、runner和`mutate.py restore`复验各控制。

长期证据都在Git管理的本目录及summary列出的依赖；七个大输出无损gzip并验证原始SHA。构建二进制可重建，不作为唯一证据。无新runtime、生产/reader/引擎/原资产不改，无新D，272项未修；完整Root、其他数据和F32旋转运算／Rig消费者继续。截图0、性能暂停，GPU周边排除。
