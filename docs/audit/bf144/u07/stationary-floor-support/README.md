# BF144-D31 静止 Walking 的地板修正

本地 ALS `AlsCharacterMovementComponent.cpp` 的 `PhysWalking` 保持水平速度并对可走地面调用 `AdjustFloorHeight`。UE `CharacterMovementComponent.cpp` 在进入 Walking 时也先投影速度、查询地板，再调整高度及绑定基座。使用真实角色的五个初始高度复采，原生 1.8cm 与 2.5cm 均修正为约 2.15cm，带内三组保留原高度，20 帧竖速全为零。

目标失败的首帧，地板扫掠距 2.4999894cm，修正扫掠无遮挡。Tnua 此帧 `displacement=None`，马达没有抵消重力，实际求解先下落 2.725mm，后置检查因已进入允许带保留该位置。静止 Walking 已确认可走地面时，沿已有竖直马达抵消物理重力，随后由现行地板扫掠按 1.9–2.4cm 范围调整。失地、穿透、顶板和基座位移由同一套定向测试复验。

用 `tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Character/B_Als_Character.B_Als_Character -CharacterAnimationProbe <spawn.request.json> -Output <output>` 可复采原生四帧乘五组，历史中途注入的读数继续保存在 `docs/audit/bf144/d31/`。
