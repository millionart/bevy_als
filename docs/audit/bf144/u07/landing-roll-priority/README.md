# 起身输入锁的落地对照

源 CS_Als_Default 的 Roll 门槛为 700 cm/s，Ragdoll 为 1000 cm/s。AAlsCharacter::NotifyLocomotionModeChanged 先检查 Ragdoll，再检查 Roll。受阻和解除后的两组控制都使用同一个 8 m/s 实际落地事件及运动快照。
