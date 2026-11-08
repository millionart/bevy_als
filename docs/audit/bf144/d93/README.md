# BF144-D93 Ragdoll硬接触的法向恢复

fresh PA确认21个body材质均为`SoftCollisionMode=None`，且非one-way、不覆盖穿透速度；fresh PhysicsSettings给世界最大pushout速度1000cm/s，相关全局覆盖关闭。两份原生输出分别与D65a和D81受跟踪记录逐字节相同，材料、请求、报告及源/目标求解片段已保存。

Avian公开的已准备contact constraint包含每点法向softness。目标只对涉及Ragdoll body的点设置源硬接触参数；源世界pushout速度由 `python3 tools/import/import_contact_pushout.py --settings docs/audit/bf144/d81/settings.native.json --native docs/audit/bf144/d93/native-contact.json --output src/als/settings/contact_pushout_speed.rs` 离线生成，并在物理帧映射到世界求解配置。已写加载head、30/60Hz、浅/深重叠及普通刚体控制。

本项只覆盖当前已证实的硬材质接触，另一侧具有源软材质时的选择逻辑、复杂多点接触、测试、消融与Playground仍待统一Gate。
