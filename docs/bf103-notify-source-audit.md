# BF103 ALS Animation Event Source Audit

This report is generated from local ALS animation uassets listed in the export manifest.

## Result

- Animations audited: 137
- Exported animations audited: 128
- Asset failures: 0
- Point notifies: 161
- Notify states: 16
- Authored sync markers: 60
- Montage segments: 9
- Event times valid: True
- Marker times valid: True
- References valid: True

## Event Classes

| Class | Count |
| --- | ---: |
| AlsAnimNotifyState_EarlyBlendOut | 6 |
| AlsAnimNotifyState_SetLocomotionAction | 9 |
| AlsAnimNotifyState_SetRootMotionScale | 1 |
| AlsAnimNotify_CameraShake | 6 |
| AlsAnimNotify_FootstepEffects | 154 |
| AlsAnimNotify_SetGroundedEntryMode | 1 |

## Animation Asset Classes

| Class | Count |
| --- | ---: |
| AnimMontage | 9 |
| AnimSequence | 128 |

## Animation Events

| Animation | Target glTF | Kind | Name | Class | Start | End | Weight | Track |
| --- | --- | --- | --- | --- | ---: | ---: | ---: | --- |
| /ALS/ALS/Animations/Actions/GetUp/A_Als_GetUp_Back.A_Als_GetUp_Back | assets/ALS/Animations/Actions/GetUp/A_Als_GetUp_Back.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.500000 | 0.500000 | 0.500000 | 1 |
| /ALS/ALS/Animations/Actions/GetUp/A_Als_GetUp_Back.A_Als_GetUp_Back | assets/ALS/Animations/Actions/GetUp/A_Als_GetUp_Back.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.066667 | 1.066667 | 0.500000 | 1 |
| /ALS/ALS/Animations/Actions/GetUp/A_Als_GetUp_Front.A_Als_GetUp_Front | assets/ALS/Animations/Actions/GetUp/A_Als_GetUp_Front.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.800000 | 0.800000 | 0.500000 | 1 |
| /ALS/ALS/Animations/Actions/GetUp/A_Als_GetUp_Front.A_Als_GetUp_Front | assets/ALS/Animations/Actions/GetUp/A_Als_GetUp_Front.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.233333 | 1.233333 | 0.500000 | 1 |
| /ALS/ALS/Animations/Actions/GetUp/AM_Als_GetUp_Back.AM_Als_GetUp_Back | None | state | BMNotifyStateMovementAction | AlsAnimNotifyState_SetLocomotionAction | 0.000000 | 1.166667 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/GetUp/AM_Als_GetUp_Front.AM_Als_GetUp_Front | None | state | BMNotifyStateMovementAction | AlsAnimNotifyState_SetLocomotionAction | 0.000000 | 1.166667 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_High.A_Als_Mantle_High | assets/ALS/Animations/Actions/Mantle/A_Als_Mantle_High.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.566667 | 0.566667 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_High.A_Als_Mantle_High | assets/ALS/Animations/Actions/Mantle/A_Als_Mantle_High.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.700000 | 1.700000 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_High.A_Als_Mantle_High | assets/ALS/Animations/Actions/Mantle/A_Als_Mantle_High.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 2.333333 | 2.333333 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Left.A_Als_Mantle_Low_Left | assets/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.333333 | 0.333333 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Left.A_Als_Mantle_Low_Left | assets/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.733333 | 0.733333 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Left.A_Als_Mantle_Low_Left | assets/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.266667 | 1.266667 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Right.A_Als_Mantle_Low_Right | assets/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.333333 | 0.333333 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Right.A_Als_Mantle_Low_Right | assets/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.733333 | 0.733333 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Right.A_Als_Mantle_Low_Right | assets/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.266667 | 1.266667 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_High.AM_Als_Mantle_High | None | state | BMNotifyStateMovementAction | AlsAnimNotifyState_SetLocomotionAction | 0.000000 | 2.066667 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_High.AM_Als_Mantle_High | None | state | BMNotifyStateEarlyBlendOut | AlsAnimNotifyState_EarlyBlendOut | 2.133333 | 2.666667 | 0.000010 | 2 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low.AM_Als_Mantle_Low | None | state | BMNotifyStateMovementAction | AlsAnimNotifyState_SetLocomotionAction | 0.000000 | 0.966667 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low.AM_Als_Mantle_Low | None | state | BMNotifyStateEarlyBlendOut | AlsAnimNotifyState_EarlyBlendOut | 1.000000 | 1.500000 | 0.000010 | 2 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_BothArms.AM_Als_Mantle_Low_BothArms | None | state | BMNotifyStateMovementAction | AlsAnimNotifyState_SetLocomotionAction | 0.000000 | 0.966667 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_BothArms.AM_Als_Mantle_Low_BothArms | None | state | BMNotifyStateEarlyBlendOut | AlsAnimNotifyState_EarlyBlendOut | 1.000000 | 1.500000 | 0.000010 | 2 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_Box.AM_Als_Mantle_Low_Box | None | state | BMNotifyStateMovementAction | AlsAnimNotifyState_SetLocomotionAction | 0.000000 | 0.966667 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_Box.AM_Als_Mantle_Low_Box | None | state | BMNotifyStateEarlyBlendOut | AlsAnimNotifyState_EarlyBlendOut | 1.000000 | 1.500000 | 0.000010 | 2 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_Left.AM_Als_Mantle_Low_Left | None | state | BMNotifyStateMovementAction | AlsAnimNotifyState_SetLocomotionAction | 0.000000 | 0.966667 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_Left.AM_Als_Mantle_Low_Left | None | state | BMNotifyStateEarlyBlendOut | AlsAnimNotifyState_EarlyBlendOut | 1.000000 | 1.500000 | 0.000010 | 2 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_Right.AM_Als_Mantle_Low_Right | None | state | BMNotifyStateMovementAction | AlsAnimNotifyState_SetLocomotionAction | 0.000000 | 0.966667 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_Right.AM_Als_Mantle_Low_Right | None | state | BMNotifyStateEarlyBlendOut | AlsAnimNotifyState_EarlyBlendOut | 1.000000 | 1.500000 | 0.000010 | 2 |
| /ALS/ALS/Animations/Actions/Roll/A_Als_Roll.A_Als_Roll | assets/ALS/Animations/Actions/Roll/A_Als_Roll.gltf | state | AlsRootMotionScale | AlsAnimNotifyState_SetRootMotionScale | 0.000000 | 1.500000 | 0.000010 | 2 |
| /ALS/ALS/Animations/Actions/Roll/A_Als_Roll.A_Als_Roll | assets/ALS/Animations/Actions/Roll/A_Als_Roll.gltf | point | AlsCameraShake | AlsAnimNotify_CameraShake | 0.100000 | 0.100000 | 0.000010 | 4 |
| /ALS/ALS/Animations/Actions/Roll/A_Als_Roll.A_Als_Roll | assets/ALS/Animations/Actions/Roll/A_Als_Roll.gltf | point | AlsCameraShake | AlsAnimNotify_CameraShake | 0.466667 | 0.466667 | 0.000010 | 4 |
| /ALS/ALS/Animations/Actions/Roll/A_Als_Roll.A_Als_Roll | assets/ALS/Animations/Actions/Roll/A_Als_Roll.gltf | point | AlsSetGroundedEntryMode | AlsAnimNotify_SetGroundedEntryMode | 0.933333 | 0.933333 | 0.000010 | 1 |
| /ALS/ALS/Animations/Actions/Roll/A_Als_Roll.A_Als_Roll | assets/ALS/Animations/Actions/Roll/A_Als_Roll.gltf | point | AlsFootstepEffects | AlsAnimNotify_FootstepEffects | 0.933333 | 0.933333 | 0.500000 | 3 |
| /ALS/ALS/Animations/Actions/Roll/A_Als_Roll.A_Als_Roll | assets/ALS/Animations/Actions/Roll/A_Als_Roll.gltf | point | AlsFootstepEffects | AlsAnimNotify_FootstepEffects | 1.000000 | 1.000000 | 0.500000 | 3 |
| /ALS/ALS/Animations/Actions/Roll/A_Als_Roll.A_Als_Roll | assets/ALS/Animations/Actions/Roll/A_Als_Roll.gltf | point | AlsFootstepEffects | AlsAnimNotify_FootstepEffects | 1.233333 | 1.233333 | 0.500000 | 3 |
| /ALS/ALS/Animations/Actions/Roll/AM_Als_Roll.AM_Als_Roll | None | state | BMNotifyStateMovementAction | AlsAnimNotifyState_SetLocomotionAction | 0.000000 | 0.933333 | 0.000010 | 1 |
| /ALS/ALS/Animations/Air/Jump/A_Als_Jump_Run_Left.A_Als_Jump_Run_Left | assets/ALS/Animations/Air/Jump/A_Als_Jump_Run_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.000000 | 0.000000 | 0.000010 | 1 |
| /ALS/ALS/Animations/Air/Jump/A_Als_Jump_Run_Right.A_Als_Jump_Run_Right | assets/ALS/Animations/Air/Jump/A_Als_Jump_Run_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.000000 | 0.000000 | 0.000010 | 1 |
| /ALS/ALS/Animations/Air/Jump/A_Als_Jump_Walk_Left.A_Als_Jump_Walk_Left | assets/ALS/Animations/Air/Jump/A_Als_Jump_Walk_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.133333 | 0.133333 | 0.000010 | 1 |
| /ALS/ALS/Animations/Air/Jump/A_Als_Jump_Walk_Right.A_Als_Jump_Walk_Right | assets/ALS/Animations/Air/Jump/A_Als_Jump_Walk_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.133333 | 0.133333 | 0.000010 | 1 |
| /ALS/ALS/Animations/Air/Land/A_Als_Land_Heavy.A_Als_Land_Heavy | assets/ALS/Animations/Air/Land/A_Als_Land_Heavy.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.100000 | 0.100000 | 0.000010 | 1 |
| /ALS/ALS/Animations/Air/Land/A_Als_Land_Heavy.A_Als_Land_Heavy | assets/ALS/Animations/Air/Land/A_Als_Land_Heavy.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.100000 | 0.100000 | 0.000010 | 2 |
| /ALS/ALS/Animations/Air/Land/A_Als_Land_Heavy_Additive.A_Als_Land_Heavy_Additive | assets/ALS/Animations/Air/Land/A_Als_Land_Heavy_Additive.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.100000 | 0.100000 | 0.000010 | 1 |
| /ALS/ALS/Animations/Air/Land/A_Als_Land_Heavy_Additive.A_Als_Land_Heavy_Additive | assets/ALS/Animations/Air/Land/A_Als_Land_Heavy_Additive.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.100000 | 0.100000 | 0.000010 | 2 |
| /ALS/ALS/Animations/Air/Land/A_Als_Land_Light.A_Als_Land_Light | assets/ALS/Animations/Air/Land/A_Als_Land_Light.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.100000 | 0.100000 | 0.000010 | 1 |
| /ALS/ALS/Animations/Air/Land/A_Als_Land_Light.A_Als_Land_Light | assets/ALS/Animations/Air/Land/A_Als_Land_Light.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.100000 | 0.100000 | 0.000010 | 2 |
| /ALS/ALS/Animations/Air/Land/A_Als_Land_Light_Additive.A_Als_Land_Light_Additive | assets/ALS/Animations/Air/Land/A_Als_Land_Light_Additive.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.100000 | 0.100000 | 0.000010 | 1 |
| /ALS/ALS/Animations/Air/Land/A_Als_Land_Light_Additive.A_Als_Land_Light_Additive | assets/ALS/Animations/Air/Land/A_Als_Land_Light_Additive.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.100000 | 0.100000 | 0.000010 | 2 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Backward.A_Als_Crouch_Walk_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.366667 | 0.366667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Backward.A_Als_Crouch_Walk_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.533333 | 0.533333 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Backward.A_Als_Crouch_Walk_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.933333 | 0.933333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Backward.A_Als_Crouch_Walk_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.100000 | 1.100000 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Forward.A_Als_Crouch_Walk_Forward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.133333 | 0.133333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Forward.A_Als_Crouch_Walk_Forward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.700000 | 0.700000 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Backward.A_Als_Crouch_Walk_Left_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.366667 | 0.366667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Backward.A_Als_Crouch_Walk_Left_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.533333 | 0.533333 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Backward.A_Als_Crouch_Walk_Left_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.933333 | 0.933333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Backward.A_Als_Crouch_Walk_Left_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.100000 | 1.100000 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Forward.A_Als_Crouch_Walk_Left_Forward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.133333 | 0.133333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Forward.A_Als_Crouch_Walk_Left_Forward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.700000 | 0.700000 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Pose.A_Als_Crouch_Walk_Pose | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Pose.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.133333 | 0.133333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Pose.A_Als_Crouch_Walk_Pose | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Pose.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.700000 | 0.700000 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Backward.A_Als_Crouch_Walk_Right_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.366667 | 0.366667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Backward.A_Als_Crouch_Walk_Right_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.533333 | 0.533333 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Backward.A_Als_Crouch_Walk_Right_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.933333 | 0.933333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Backward.A_Als_Crouch_Walk_Right_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.100000 | 1.100000 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Forward.A_Als_Crouch_Walk_Right_Forward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.133333 | 0.133333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Forward.A_Als_Crouch_Walk_Right_Forward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.700000 | 0.700000 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint.A_Als_Sprint | assets/ALS/Animations/Grounded/Sprint/A_Als_Sprint.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.066667 | 0.066667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint.A_Als_Sprint | assets/ALS/Animations/Grounded/Sprint/A_Als_Sprint.gltf | point | AlsCameraShake | AlsAnimNotify_CameraShake | 0.066667 | 0.066667 | 0.500000 | 2 |
| /ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint.A_Als_Sprint | assets/ALS/Animations/Grounded/Sprint/A_Als_Sprint.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.366667 | 0.366667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint.A_Als_Sprint | assets/ALS/Animations/Grounded/Sprint/A_Als_Sprint.gltf | point | AlsCameraShake | AlsAnimNotify_CameraShake | 0.366667 | 0.366667 | 0.500000 | 2 |
| /ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.A_Als_Sprint_Acceleration | assets/ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.066667 | 0.066667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.A_Als_Sprint_Acceleration | assets/ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.gltf | point | AlsCameraShake | AlsAnimNotify_CameraShake | 0.066667 | 0.066667 | 0.500000 | 2 |
| /ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.A_Als_Sprint_Acceleration | assets/ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.366667 | 0.366667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.A_Als_Sprint_Acceleration | assets/ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.gltf | point | AlsCameraShake | AlsAnimNotify_CameraShake | 0.366667 | 0.366667 | 0.500000 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Backward.A_Als_Run_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.066667 | 0.066667 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Backward.A_Als_Run_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.166667 | 0.166667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Backward.A_Als_Run_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.400000 | 0.400000 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Backward.A_Als_Run_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.500000 | 0.500000 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward.A_Als_Run_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.066667 | 0.066667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward.A_Als_Run_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.466667 | 0.466667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward_Pose.A_Als_Run_Forward_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward_Pose.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.066667 | 0.066667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward_Pose.A_Als_Run_Forward_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward_Pose.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.466667 | 0.466667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Backward.A_Als_Run_Left_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.066667 | 0.066667 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Backward.A_Als_Run_Left_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.166667 | 0.166667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Backward.A_Als_Run_Left_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.400000 | 0.400000 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Backward.A_Als_Run_Left_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.500000 | 0.500000 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Forward.A_Als_Run_Left_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.066667 | 0.066667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Forward.A_Als_Run_Left_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.466667 | 0.466667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Pose.A_Als_Run_Left_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Pose.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.066667 | 0.066667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Pose.A_Als_Run_Left_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Pose.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.466667 | 0.466667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Backward.A_Als_Run_Right_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.066667 | 0.066667 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Backward.A_Als_Run_Right_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.166667 | 0.166667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Backward.A_Als_Run_Right_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.400000 | 0.400000 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Backward.A_Als_Run_Right_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.500000 | 0.500000 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Forward.A_Als_Run_Right_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.066667 | 0.066667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Forward.A_Als_Run_Right_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.466667 | 0.466667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Pose.A_Als_Run_Right_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Pose.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.066667 | 0.066667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Pose.A_Als_Run_Right_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Pose.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.466667 | 0.466667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Backward.A_Als_Walk_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.433333 | 0.433333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Backward.A_Als_Walk_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.533333 | 0.533333 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Backward.A_Als_Walk_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.000000 | 1.000000 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Backward.A_Als_Walk_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.100000 | 1.100000 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward.A_Als_Walk_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.133333 | 0.133333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward.A_Als_Walk_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.700000 | 0.700000 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward_Pose.A_Als_Walk_Forward_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward_Pose.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.133333 | 0.133333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward_Pose.A_Als_Walk_Forward_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward_Pose.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.700000 | 0.700000 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Backward.A_Als_Walk_Left_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.433333 | 0.433333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Backward.A_Als_Walk_Left_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.533333 | 0.533333 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Backward.A_Als_Walk_Left_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.000000 | 1.000000 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Backward.A_Als_Walk_Left_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.100000 | 1.100000 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Forward.A_Als_Walk_Left_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.133333 | 0.133333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Forward.A_Als_Walk_Left_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.700000 | 0.700000 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Pose.A_Als_Walk_Left_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Pose.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.133333 | 0.133333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Pose.A_Als_Walk_Left_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Pose.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.700000 | 0.700000 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Backward.A_Als_Walk_Right_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.433333 | 0.433333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Backward.A_Als_Walk_Right_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.533333 | 0.533333 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Backward.A_Als_Walk_Right_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.000000 | 1.000000 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Backward.A_Als_Walk_Right_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Backward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.100000 | 1.100000 | 0.300000 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Forward.A_Als_Walk_Right_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.133333 | 0.133333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Forward.A_Als_Walk_Right_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Forward.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.700000 | 0.700000 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Pose.A_Als_Walk_Right_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Pose.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.133333 | 0.133333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Pose.A_Als_Walk_Right_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Pose.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.700000 | 0.700000 | 0.300000 | 1 |
| /ALS/ALS/Animations/RotateInPlace/A_Als_Crouch_Rotate_90_Left.A_Als_Crouch_Rotate_90_Left | assets/ALS/Animations/RotateInPlace/A_Als_Crouch_Rotate_90_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.433333 | 0.433333 | 0.500000 | 1 |
| /ALS/ALS/Animations/RotateInPlace/A_Als_Crouch_Rotate_90_Left.A_Als_Crouch_Rotate_90_Left | assets/ALS/Animations/RotateInPlace/A_Als_Crouch_Rotate_90_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.733333 | 0.733333 | 0.500000 | 1 |
| /ALS/ALS/Animations/RotateInPlace/A_Als_Crouch_Rotate_90_Right.A_Als_Crouch_Rotate_90_Right | assets/ALS/Animations/RotateInPlace/A_Als_Crouch_Rotate_90_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.433333 | 0.433333 | 0.500000 | 1 |
| /ALS/ALS/Animations/RotateInPlace/A_Als_Crouch_Rotate_90_Right.A_Als_Crouch_Rotate_90_Right | assets/ALS/Animations/RotateInPlace/A_Als_Crouch_Rotate_90_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.733333 | 0.733333 | 0.500000 | 1 |
| /ALS/ALS/Animations/RotateInPlace/A_Als_Rotate_90_Left.A_Als_Rotate_90_Left | assets/ALS/Animations/RotateInPlace/A_Als_Rotate_90_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.500000 | 0.500000 | 0.500000 | 1 |
| /ALS/ALS/Animations/RotateInPlace/A_Als_Rotate_90_Left.A_Als_Rotate_90_Left | assets/ALS/Animations/RotateInPlace/A_Als_Rotate_90_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.966667 | 0.966667 | 0.500000 | 1 |
| /ALS/ALS/Animations/RotateInPlace/A_Als_Rotate_90_Right.A_Als_Rotate_90_Right | assets/ALS/Animations/RotateInPlace/A_Als_Rotate_90_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.500000 | 0.500000 | 0.500000 | 1 |
| /ALS/ALS/Animations/RotateInPlace/A_Als_Rotate_90_Right.A_Als_Rotate_90_Right | assets/ALS/Animations/RotateInPlace/A_Als_Rotate_90_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.966667 | 0.966667 | 0.500000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_Crouch_DynamicTransition_Left.A_Als_Crouch_DynamicTransition_Left | assets/ALS/Animations/Transitions/A_Als_Crouch_DynamicTransition_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.566667 | 0.566667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_Crouch_DynamicTransition_Right.A_Als_Crouch_DynamicTransition_Right | assets/ALS/Animations/Transitions/A_Als_Crouch_DynamicTransition_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.566667 | 0.566667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_Crouch_Transition_Left.A_Als_Crouch_Transition_Left | assets/ALS/Animations/Transitions/A_Als_Crouch_Transition_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.833333 | 0.833333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_Crouch_Transition_Left.A_Als_Crouch_Transition_Left | assets/ALS/Animations/Transitions/A_Als_Crouch_Transition_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.366667 | 1.366667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_Crouch_Transition_Right.A_Als_Crouch_Transition_Right | assets/ALS/Animations/Transitions/A_Als_Crouch_Transition_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.833333 | 0.833333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_Crouch_Transition_Right.A_Als_Crouch_Transition_Right | assets/ALS/Animations/Transitions/A_Als_Crouch_Transition_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.366667 | 1.366667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_CrouchToStand.A_Als_CrouchToStand | assets/ALS/Animations/Transitions/A_Als_CrouchToStand.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.600000 | 0.600000 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_CrouchToStand.A_Als_CrouchToStand | assets/ALS/Animations/Transitions/A_Als_CrouchToStand.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.900000 | 0.900000 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_Stand_DynamicTransition_Left.A_Als_Stand_DynamicTransition_Left | assets/ALS/Animations/Transitions/A_Als_Stand_DynamicTransition_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.566667 | 0.566667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_Stand_DynamicTransition_Right.A_Als_Stand_DynamicTransition_Right | assets/ALS/Animations/Transitions/A_Als_Stand_DynamicTransition_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.566667 | 0.566667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_Stand_Transition_Left.A_Als_Stand_Transition_Left | assets/ALS/Animations/Transitions/A_Als_Stand_Transition_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.833333 | 0.833333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_Stand_Transition_Left.A_Als_Stand_Transition_Left | assets/ALS/Animations/Transitions/A_Als_Stand_Transition_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.366667 | 1.366667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_Stand_Transition_Right.A_Als_Stand_Transition_Right | assets/ALS/Animations/Transitions/A_Als_Stand_Transition_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.833333 | 0.833333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_Stand_Transition_Right.A_Als_Stand_Transition_Right | assets/ALS/Animations/Transitions/A_Als_Stand_Transition_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.366667 | 1.366667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_StandToCrouch.A_Als_StandToCrouch | assets/ALS/Animations/Transitions/A_Als_StandToCrouch.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.433333 | 0.433333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_StandToCrouch.A_Als_StandToCrouch | assets/ALS/Animations/Transitions/A_Als_StandToCrouch.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.733333 | 0.733333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_Stop_Left.A_Als_Stop_Left | assets/ALS/Animations/Transitions/A_Als_Stop_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.833333 | 0.833333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_Stop_Left.A_Als_Stop_Left | assets/ALS/Animations/Transitions/A_Als_Stop_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.366667 | 1.366667 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_Stop_Right.A_Als_Stop_Right | assets/ALS/Animations/Transitions/A_Als_Stop_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.833333 | 0.833333 | 0.300000 | 1 |
| /ALS/ALS/Animations/Transitions/A_Als_Stop_Right.A_Als_Stop_Right | assets/ALS/Animations/Transitions/A_Als_Stop_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.366667 | 1.366667 | 0.300000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Left.A_Als_Crouch_Turn_180_Left | assets/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.533333 | 0.533333 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Left.A_Als_Crouch_Turn_180_Left | assets/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.133333 | 1.133333 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Left.A_Als_Crouch_Turn_180_Left | assets/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.633333 | 1.633333 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Left.A_Als_Crouch_Turn_180_Left | assets/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 2.233333 | 2.233333 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Right.A_Als_Crouch_Turn_180_Right | assets/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.533333 | 0.533333 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Right.A_Als_Crouch_Turn_180_Right | assets/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.133333 | 1.133333 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Right.A_Als_Crouch_Turn_180_Right | assets/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.633333 | 1.633333 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Right.A_Als_Crouch_Turn_180_Right | assets/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_180_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 2.133333 | 2.133333 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_90_Left.A_Als_Crouch_Turn_90_Left | assets/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_90_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.700000 | 0.700000 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_90_Left.A_Als_Crouch_Turn_90_Left | assets/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_90_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.233333 | 1.233333 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_90_Left.A_Als_Crouch_Turn_90_Left | assets/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_90_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.733333 | 1.733333 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_90_Right.A_Als_Crouch_Turn_90_Right | assets/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_90_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.700000 | 0.700000 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_90_Right.A_Als_Crouch_Turn_90_Right | assets/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_90_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.233333 | 1.233333 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_90_Right.A_Als_Crouch_Turn_90_Right | assets/ALS/Animations/TurnInPlace/A_Als_Crouch_Turn_90_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.733333 | 1.733333 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Turn_180_Left.A_Als_Turn_180_Left | assets/ALS/Animations/TurnInPlace/A_Als_Turn_180_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.833333 | 0.833333 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Turn_180_Left.A_Als_Turn_180_Left | assets/ALS/Animations/TurnInPlace/A_Als_Turn_180_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.300000 | 1.300000 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Turn_180_Left.A_Als_Turn_180_Left | assets/ALS/Animations/TurnInPlace/A_Als_Turn_180_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.800000 | 1.800000 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Turn_180_Right.A_Als_Turn_180_Right | assets/ALS/Animations/TurnInPlace/A_Als_Turn_180_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.833333 | 0.833333 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Turn_180_Right.A_Als_Turn_180_Right | assets/ALS/Animations/TurnInPlace/A_Als_Turn_180_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.300000 | 1.300000 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Turn_180_Right.A_Als_Turn_180_Right | assets/ALS/Animations/TurnInPlace/A_Als_Turn_180_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.800000 | 1.800000 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Turn_90_Left.A_Als_Turn_90_Left | assets/ALS/Animations/TurnInPlace/A_Als_Turn_90_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.833333 | 0.833333 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Turn_90_Left.A_Als_Turn_90_Left | assets/ALS/Animations/TurnInPlace/A_Als_Turn_90_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.366667 | 1.366667 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Turn_90_Left.A_Als_Turn_90_Left | assets/ALS/Animations/TurnInPlace/A_Als_Turn_90_Left.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.766667 | 1.766667 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Turn_90_Right.A_Als_Turn_90_Right | assets/ALS/Animations/TurnInPlace/A_Als_Turn_90_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 0.833333 | 0.833333 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Turn_90_Right.A_Als_Turn_90_Right | assets/ALS/Animations/TurnInPlace/A_Als_Turn_90_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.366667 | 1.366667 | 0.500000 | 1 |
| /ALS/ALS/Animations/TurnInPlace/A_Als_Turn_90_Right.A_Als_Turn_90_Right | assets/ALS/Animations/TurnInPlace/A_Als_Turn_90_Right.gltf | point | BMAnimNotifyFootstep | AlsAnimNotify_FootstepEffects | 1.766667 | 1.766667 | 0.500000 | 1 |

## Authored Sync Markers

| Animation | Target glTF | Marker | Time | Track |
| --- | --- | --- | ---: | ---: |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Backward.A_Als_Crouch_Walk_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Backward.gltf | Right | 0.500000 | 2 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Backward.A_Als_Crouch_Walk_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Backward.gltf | Left | 1.066667 | 2 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Forward.A_Als_Crouch_Walk_Forward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Forward.gltf | Left | 0.091667 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Forward.A_Als_Crouch_Walk_Forward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Forward.gltf | Right | 0.650000 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Backward.A_Als_Crouch_Walk_Left_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Backward.gltf | Right | 0.466667 | 2 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Backward.A_Als_Crouch_Walk_Left_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Backward.gltf | Left | 1.041667 | 2 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Forward.A_Als_Crouch_Walk_Left_Forward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Forward.gltf | Left | 0.091667 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Forward.A_Als_Crouch_Walk_Left_Forward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Left_Forward.gltf | Right | 0.591667 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Pose.A_Als_Crouch_Walk_Pose | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Pose.gltf | Left | 0.091667 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Pose.A_Als_Crouch_Walk_Pose | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Pose.gltf | Right | 0.650000 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Backward.A_Als_Crouch_Walk_Right_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Backward.gltf | Right | 0.483333 | 2 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Backward.A_Als_Crouch_Walk_Right_Backward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Backward.gltf | Left | 1.041667 | 2 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Forward.A_Als_Crouch_Walk_Right_Forward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Forward.gltf | Left | 0.016667 | 1 |
| /ALS/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Forward.A_Als_Crouch_Walk_Right_Forward | assets/ALS/Animations/Grounded/Crouch/A_Als_Crouch_Walk_Right_Forward.gltf | Right | 0.650000 | 1 |
| /ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint.A_Als_Sprint | assets/ALS/Animations/Grounded/Sprint/A_Als_Sprint.gltf | Left | 0.016667 | 2 |
| /ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint.A_Als_Sprint | assets/ALS/Animations/Grounded/Sprint/A_Als_Sprint.gltf | Right | 0.316667 | 2 |
| /ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.A_Als_Sprint_Acceleration | assets/ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.gltf | Left | 0.016667 | 2 |
| /ALS/ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.A_Als_Sprint_Acceleration | assets/ALS/Animations/Grounded/Sprint/A_Als_Sprint_Acceleration.gltf | Right | 0.316667 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Backward.A_Als_Run_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Backward.gltf | Left | 0.050000 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Backward.A_Als_Run_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Backward.gltf | Right | 0.383333 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward.A_Als_Run_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward.gltf | Left | 0.033333 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward.A_Als_Run_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward.gltf | Right | 0.433333 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward_Pose.A_Als_Run_Forward_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward_Pose.gltf | Left | 0.033333 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward_Pose.A_Als_Run_Forward_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Forward_Pose.gltf | Right | 0.433333 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Backward.A_Als_Run_Left_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Backward.gltf | Right | 0.316667 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Backward.A_Als_Run_Left_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Backward.gltf | Left | 0.650000 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Forward.A_Als_Run_Left_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Forward.gltf | Left | 0.033333 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Forward.A_Als_Run_Left_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Forward.gltf | Right | 0.450000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Pose.A_Als_Run_Left_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Pose.gltf | Left | 0.033333 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Pose.A_Als_Run_Left_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Left_Pose.gltf | Right | 0.450000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Backward.A_Als_Run_Right_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Backward.gltf | Right | 0.316667 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Backward.A_Als_Run_Right_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Backward.gltf | Left | 0.650000 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Forward.A_Als_Run_Right_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Forward.gltf | Left | 0.050000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Forward.A_Als_Run_Right_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Forward.gltf | Right | 0.433333 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Pose.A_Als_Run_Right_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Pose.gltf | Left | 0.050000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Pose.A_Als_Run_Right_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Run_Right_Pose.gltf | Right | 0.433333 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Backward.A_Als_Walk_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Backward.gltf | Right | 0.516667 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Backward.A_Als_Walk_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Backward.gltf | Left | 1.083333 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward.A_Als_Walk_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward.gltf | Left | 0.083333 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward.A_Als_Walk_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward.gltf | Right | 0.650000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward_Pose.A_Als_Walk_Forward_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward_Pose.gltf | Left | 0.083333 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward_Pose.A_Als_Walk_Forward_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Forward_Pose.gltf | Right | 0.650000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Backward.A_Als_Walk_Left_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Backward.gltf | Right | 0.508333 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Backward.A_Als_Walk_Left_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Backward.gltf | Left | 1.058333 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Forward.A_Als_Walk_Left_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Forward.gltf | Left | 0.083333 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Forward.A_Als_Walk_Left_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Forward.gltf | Right | 0.658333 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Pose.A_Als_Walk_Left_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Pose.gltf | Left | 0.083333 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Pose.A_Als_Walk_Left_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Left_Pose.gltf | Right | 0.658333 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Backward.A_Als_Walk_Right_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Backward.gltf | Right | 0.491667 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Backward.A_Als_Walk_Right_Backward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Backward.gltf | Left | 1.075000 | 2 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Forward.A_Als_Walk_Right_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Forward.gltf | Left | 0.091667 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Forward.A_Als_Walk_Right_Forward | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Forward.gltf | Right | 0.650000 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Pose.A_Als_Walk_Right_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Pose.gltf | Left | 0.091667 | 1 |
| /ALS/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Pose.A_Als_Walk_Right_Pose | assets/ALS/Animations/Grounded/WalkRun/A_Als_Walk_Right_Pose.gltf | Right | 0.650000 | 1 |
| /ALS/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Run_Arms.A_Als_Rifle_Run_Arms | assets/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Run_Arms.gltf | Left | 0.033401 | 0 |
| /ALS/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Run_Arms.A_Als_Rifle_Run_Arms | assets/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Run_Arms.gltf | Right | 0.433038 | 0 |
| /ALS/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Sprint_Arms.A_Als_Rifle_Sprint_Arms | assets/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Sprint_Arms.gltf | Left | 0.053793 | 0 |
| /ALS/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Sprint_Arms.A_Als_Rifle_Sprint_Arms | assets/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Sprint_Arms.gltf | Right | 0.353976 | 0 |
| /ALS/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Sprint_Impulse_Arms.A_Als_Rifle_Sprint_Impulse_Arms | assets/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Sprint_Impulse_Arms.gltf | Left | 0.053793 | 0 |
| /ALS/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Sprint_Impulse_Arms.A_Als_Rifle_Sprint_Impulse_Arms | assets/ALS/Animations/Overlays/Rifle/A_Als_Rifle_Sprint_Impulse_Arms.gltf | Right | 0.353976 | 0 |

## Montage Segments

| Montage | Slot | Animation | Start position | Animation start | Animation end | Rate | Loops |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |
| /ALS/ALS/Animations/Actions/GetUp/AM_Als_GetUp_Back.AM_Als_GetUp_Back | PostLocomotion | None//ALS/ALS/Animations/Actions/GetUp/A_Als_GetUp_Back/A_Als_GetUp_Back | 0.000000 | 0.000000 | 1.500000 | 1.000000 | 1 |
| /ALS/ALS/Animations/Actions/GetUp/AM_Als_GetUp_Front.AM_Als_GetUp_Front | PostLocomotion | None//ALS/ALS/Animations/Actions/GetUp/A_Als_GetUp_Front/A_Als_GetUp_Front | 0.000000 | 0.000000 | 1.500000 | 1.000000 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_High.AM_Als_Mantle_High | PostLocomotion | None//ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_High/A_Als_Mantle_High | 0.000000 | 0.000000 | 2.666667 | 1.000000 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low.AM_Als_Mantle_Low | PostLocomotion | None//ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Right/A_Als_Mantle_Low_Right | 0.000000 | 0.000000 | 1.500000 | 1.000000 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_BothArms.AM_Als_Mantle_Low_BothArms | PostLocomotion | None//ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Right/A_Als_Mantle_Low_Right | 0.000000 | 0.000000 | 1.500000 | 1.000000 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_Box.AM_Als_Mantle_Low_Box | PostLocomotion | None//ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Right/A_Als_Mantle_Low_Right | 0.000000 | 0.000000 | 1.500000 | 1.000000 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_Left.AM_Als_Mantle_Low_Left | PostLocomotion | None//ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Left/A_Als_Mantle_Low_Left | 0.000000 | 0.000000 | 1.500000 | 1.000000 | 1 |
| /ALS/ALS/Animations/Actions/Mantle/AM_Als_Mantle_Low_Right.AM_Als_Mantle_Low_Right | PostLocomotion | None//ALS/ALS/Animations/Actions/Mantle/A_Als_Mantle_Low_Right/A_Als_Mantle_Low_Right | 0.000000 | 0.000000 | 1.500000 | 1.000000 | 1 |
| /ALS/ALS/Animations/Actions/Roll/AM_Als_Roll.AM_Als_Roll | PostLocomotion | None//ALS/ALS/Animations/Actions/Roll/A_Als_Roll/A_Als_Roll | 0.000000 | 0.000000 | 1.500000 | 1.000000 | 1 |
