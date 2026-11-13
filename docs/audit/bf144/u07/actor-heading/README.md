# 原始动画反馈的角色朝向

原生 AnimInstanceProxy 从 Actor 取根 ComponentToWorld，ALS RefreshLocomotion 直接读取其中的四元数。目标旧出生物理注册从仿射重新分解一次，provider 再分解一次。固定 −90° 真实入口的 before-history.json.gz 有89行，第一次角色 Transform 与物理 Rotation 已不对称，第一次活动步重建恢复对称后，provider 读数仍经一次分解。定向红测分别覆盖实际公开出生和无父角色图输入。

复采时解压应用 observer-hook.patch.gz，把本目录 observer.rs 放到钩子引用路径，用相邻 crouch-matched-input/replay.ps1 通过正式入口。取证后按原字节恢复正式入口文件。
