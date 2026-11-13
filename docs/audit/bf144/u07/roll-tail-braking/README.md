# Roll 尾段普通制动

原生 UE ApplyVelocityBraking 从100cm/s、50ms和1500cm/s²得到24.99999888241291cm/s。运行时的下一速度独立投影为0.24999999m/s，物理 motor 的 boost 独立做F32差。反向从boost相加重建该速度会得到0.25m/s。定向复验同时核对真实尾段恢复普通转向。
