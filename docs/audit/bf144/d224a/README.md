# D224a 当前平台压缩资产

UE `UAnimSequence::CacheDerivedDataForCurrentPlatform` 完成当前平台缓存后，以实际网格的 `FBoneContainer` 调用 `ShouldUseRawDataForPoseExtraction`。读取当前 `FCompressedAnimSequence` 的压缩字节、track-to-skeleton 和 `IndexedCurveNames`。骨骼编号按各网格所用Skeleton保存，同时记录编解码所用Skeleton参考姿态和RequiredBones初始姿态。

通知清单和安装的 `.anim.ron` 一一对应。Character 的 127 个序列均为 ACL，包含 85 普通、15 local additive、27 mesh rotation additive。Bow 的 1 个序列使用 PerTrack，无曲线，原生时长为 1 秒。两网格合批被原生命令拒绝，按安装描述符选择网格后各自通过。

`summary.json` 核对整批资产和同一 Crouch 序列的单次压缩读取。Crouch 首键与相同时点的raw姿态在VB foot_r有 0.00168479 的最大旋转分量差。Bow 的5个原生时点在spine_down_02持续有 0.00119831 cm 的最大平移分量差。

```sh
d=docs/audit/bf144/d224a
reader=tools/run_ue_source_reader.ps1
pwsh -NoProfile -File "$reader" -Asset /ALS/ALS/Character/SKM_Als -SequenceCompressionExport "$d/character.request.json" -Output "$d/character.native.json"
pwsh -NoProfile -File "$reader" -Asset /ALS/ALS/OverlayObjects/Bow/SKM_Als_Bow -SequenceCompressionExport "$d/bow.request.json" -Output "$d/bow.native.json"
python "$d/replay_capture.py"
pwsh -NoProfile -File "$reader" -SelfTest
```

回放使用 Git 中的 gzip 原生记录及解压 SHA-256。重采的原始输出先与同名 `.native.sha256` 对照。分组请求可用 `tools/import/capture_als_sequence_compression.py --mesh <UE object path>` 重新生成。其 `--native` 参数核验新读数。
