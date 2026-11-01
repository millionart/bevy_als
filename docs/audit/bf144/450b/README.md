# BF144.450b 首Head delta X的数据与参考因素

依据.a三个fresh原生模式和实际ref/absolute/difference/public端点，内存clone原Forward clip的31个translation keys，仅key15/time.5的pelvis X改为当前ref X+native compressed delta的米制投影。所有原key均对loaded public sampler校准，所有其它key和Pose字段逐值保持；仅以公共Reflect改Forward sample0 caller的clip handle，其余ClipNode配置、Stand/Idle/其它caller保持。

原Playground184个实际half-phase差分恢复compressed X3.921628012903966e−5，首RigSource与最终Root X也精确恢复；删除唯一handle赋值、保clone与key重建后全部回原1.7007637193685825e−30。其它运动输入、clock、pose通道、motor及后相机精确保持。跨a/b仅rows唯一camera本地Entity ID整体+1，原值保留并按稳定唯一角色比较，其余字段精确。

raw参考因素独立：source raw absolute与raw Stand ref X同1.7007638171697125e−28cm，raw delta0；目标actual reference X0、absolute X1.7007637193685825e−30。只将reference X换成source物理投影1.7007639074476786e−30后，公开核保留−1.88079096131566e−37，不归零。初checker错误假设会归零，实际失败后核对物理与导出单位阶段，改为严格F32差分预期，不增加容差。同float导出系数转换raw ref会等于目标absolute X并得到0，故D235参考数据因素与剩余absolute单位因素D275分开；本例compressed delta的两种单位投影恰好相同，无额外单位恢复分支。大幅数据差归D224，不据此把所有姿态余差归压缩。

一次初始化guard移除后，第二次修改输入不再满足原31键断言；四坏route/X/repeat/input反例拒。三源码bytes/SHA恢复、同target六库重编后，全部controlled新旧数组及check字节精确，functional两次通过。396身份/六库/原main及生产、reader、原gltf/bin保持。大参数历史无损gzip/raw SHA、原始失败与恢复、身份和manifest均长期Git管理。

只扩D224/D235/D275当前consumer，无新编号，272未修。复验用已有输入 `pwsh docs/audit/bf144/450b/run_python.ps1 -Name check -Case restored`；新真实运行用新Label。完整Root其它旋转/位置/曲线残差继续，截图0/性能暂停，GPU周边排除。
