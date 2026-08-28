# BF-Char16 Parameter Audit

## Evidence baseline

- ALS plugin source: `AlsCharacter.cpp`, `AlsCharacter_Actions.cpp`, `AlsCharacterMovementComponent.cpp`, `AlsCharacterExample.cpp`, `AlsUtility.cpp`, `AlsCameraComponent.cpp` and their settings headers.
- UE 5.8 engine source: `GameplayTagsManager.cpp`, `GameplayTagContainer.h`, `GameModeBase.cpp`, `LevelActor.cpp`, `CharacterMovementComponent.cpp` and `PlayerStart.cpp`.
- Unreal MCP: `B_Als_Character`, `CS_Als_Default`, `MS_Als_Normal`, `AIS_Als_Default`, all input actions, `PA_Als`, HUD/overlay widgets and the sprint camera shake.

## Status

| Status | Meaning |
| --- | --- |
| Exact | Current runtime value and consumer match the extracted source contract. |
| Corrected | BF-Char16 changed the current implementation to the extracted source value. |
| Follow-up | The mismatch is recorded and assigned to a later BF item. |
| Adapter | Bevy/Tnua needs an implementation-specific value with no direct UE scalar equivalent. |

## Input actions and mappings

| Item | ALS source | BF-Char16 result | Status |
| --- | --- | --- | --- |
| Move, Look, LookMouse | Axis2D, no trigger | Axis2D actions with no trigger | Exact |
| Jump | Pressed and Released, threshold 0.5 | Existing press/release bridge | Exact |
| Crouch, Walk, Ragdoll, RotationMode, ViewMode, SwitchShoulder, Slomo, ToggleUI | Pressed, threshold 0.5 | Existing press edge bridge | Exact |
| Aim | Pressed and Released, threshold 0.5 | Existing held bridge | Exact |
| Sprint | Hold 0.1 s, one-shot, unaffected by dilation; Released, threshold 0.5 | Hold 0.1 s one-shot plus raw-held latch to preserve sprint until release | Corrected |
| Roll | Tap release window 0.3 s, unaffected by dilation, threshold 0.5 | Tap 0.3 s and FIRE-only roll edge | Corrected |
| Next/Previous overlay | Down, threshold 0.5 | Down trigger and Fired-state menu bridge | Corrected |
| Keyboard mappings | WASD, arrows, Space, Left Ctrl, Left Shift, RMB, Left Alt, X, 1, 2, C, Q, Z and Tab | Same bindings | Exact |
| Gamepad mappings | Move LS, Look RS, Jump A, Crouch LSB, Sprint/Roll B, Aim LB, Walk RB, Ragdoll X, rotation D-pad Left, view D-pad Right, shoulder RSB, menu Y, Slomo Select, UI Start | Aim changed to LeftTrigger and Walk changed to RightTrigger; other mappings unchanged | Corrected |

The `tools/audit_input_actions.py` MCP probe enumerates all eighteen source actions. Unit tests assert the complete default map, trigger types, thresholds, sprint latch and tap behavior. The focused runtime gate logs `tap_secs=0.300`, `hold_secs=0.100`, `one_shot=true` and both latch/release results.

## Character, CMC and movement

| Item | ALS source | Current result | Status |
| --- | --- | --- | --- |
| Initial character state | ViewDirection, Running, Standing, ThirdPerson, aim false | Bundle now creates ViewDirection, Running and Standing; third person is the absence of `AlsFirstPersonView` | Corrected |
| Standing capsule | radius 30 cm, half height 90 cm | 0.30 m and 0.90 m | Exact |
| Mesh placement | Z -92 cm, yaw -90 degrees | Avatar offset and rotation match | Exact |
| Crouched capsule | half height 56 cm | 0.56 m, including physical collider and float offset | Corrected |
| Base CMC values | gravity 1, step 45 cm, jump Z 420 cm/s, acceleration 2000 cm/s2, braking 1500 cm/s2, friction 4 | 1, 0.45 m, 4.2 m/s, 20 m/s2, 15 m/s2 and 4 | Exact values; curve consumption remains Follow-up |
| Walkable slope | 44.765083 degrees, cosine 0.71 | Tnua slope and prediction acceptance changed to those values | Corrected |
| Floor range | min 1.9 cm, max 2.4 cm | shared lower clearance 0.019 m; Tnua float target 0.024 m | Corrected |
| Standing gait table | Velocity/View 175/375/650 cm/s; Aiming 165/375/650 cm/s | normal and aiming tables are separate; physical config consumes the selected table | Corrected |
| Crouched gait data | 150/200/300 cm/s for every rotation mode; default allowed gait is Walking | table is stored; active crouched cap remains 1.50 m/s | Corrected for default runtime path |
| Air control | 0.15, low-speed boost x2 below 25 cm/s | Tnua air acceleration is 3 m/s2 or 6 m/s2 below 0.25 m/s | Corrected |
| Free-fall and jump extras | GravityScale 1, no CMC-equivalent extra gravity, JumpMaxHoldTime 0, JumpMaxCount 1 | Tnua free-fall and jump extra gravities set to zero; coyote and input buffer set to zero | Corrected |
| Sprint/aim rotation | `SprintHasPriorityOverAiming=true`, `RotateToVelocityWhenSprinting=true` | grounded third-person sprint with movement becomes VelocityDirection; first-person becomes ViewDirection; release restores aim/free mode | Corrected |
| Rotation rates | Velocity 800, View 500, Aiming 1000 degrees/s; in-air half-lives 0.2/0.1 | Existing rate values retained | Exact scalar values |
| Aiming yaw limit | 70 degrees | No runtime clamp | Follow-up: BF-Char17 |
| Acceleration/braking/friction curves | CMC refreshes these from ALS curve data by gait amount | Current Tnua drives only base acceleration; braking/friction are not physical controller inputs | Follow-up: BF-Char17 |
| Ground prediction geometry | capsule sweep, 150-2000 cm, slope cosine 0.71 | velocity/distance/slope values match; query is still a ray | Follow-up: BF-Char17 |
| Tnua spring and damping | no direct CMC scalar counterpart | spring 400 and damping 1.2 are adapter values | Adapter; prove by BF-Char23 schedule/runtime gate |

## Actions, mantle, stop and Foot IK

| Item | ALS source | Current result | Status |
| --- | --- | --- | --- |
| Roll clip | 1.5 s animation at 1.3 rate, blend in 0.1 s, blend out 0.3 s | procedural lifetime changed to 1.5/1.3 s with 0.1/0.3 weights | Corrected |
| Roll displacement and cooldown | source comes from root motion and montage lifecycle | 3 m procedural distance and 0.35 s cooldown have no extracted source scalar | Follow-up: BF-Char18 |
| Mantle | Grounded and InAir use different trace height/reach/target parameters; high is strictly above 125 cm | current single procedural trace differs | Follow-up: BF-Char18 |
| QuickStop and DynamicTransition | source uses explicit blend, start and play-rate settings | current stop match has hard-coded values and Dynamic rate 1.0 | Follow-up: BF-Char18 |
| Foot IK and lock | source includes 150 cm/s smooth threshold, 90/40 degree limits, 8 cm dynamic threshold, 0.2 blend and 1.5 rate | current analytic probes and interpolation fields do not map all source settings | Follow-up: BF-Char18 |
| Turn in place | activation maximum 0.75 s | changed from 1.0 s to 0.75 s | Corrected |
| First-person rotate-in-place threshold | 65 degrees | third-person 50 degrees is present; first-person branch is absent | Follow-up: BF-Char18 |

## Camera, HUD and overlay

| Item | ALS source | Current result | Status |
| --- | --- | --- | --- |
| FOV | first/third 90 degrees, clamp 5-175 | same values | Exact |
| Sprint FOV | sprint shake FOV amplitude 0 | no sprint FOV producer | Exact; no FOV kick should be added |
| Sprint camera shake | duration 0.2 s, blend 0.1/0.1 s, 0.1 degree rotation and 1 cm location amplitudes | no shake runtime | Follow-up: BF-Char20 |
| Camera trace radius | 15 cm Visibility sweep | 0.15 m radius; no source-channel equivalent | Follow-up: BF-Char20 |
| Curves and camera transforms | 13 curve consumers, socket midpoint pivot, trace override, lag, first-person override, unscaled time and 200 cm teleport reset | current static arms/pivots and partial FOV only | Follow-up: BF-Char20 |
| HUD layout | 340 px width, 25 px top/right, 8/4 px gaps, 12/10 px font and source input labels | same values | Exact |
| HUD colors | UE FLinearColor enabled/disabled/accent/text/borders | changed all source colors to linear Bevy colors | Corrected |
| Overlay option sizes/colors/dilation | 200x25, pad 4, selected .15/.15/.15/.5, normal .015/.015/.015/.5, dilation .35 | same linear values and dilation | Exact |
| Overlay tag/menu ordering | child tree is lexical Barrel-to-Torch; manager's LIFO traversal returns Torch-to-Barrel and widget directly iterates it | internal source tree and manager traversal are modeled explicitly; displayed visual order remains Torch-to-Barrel | Corrected |
| Overlay menu anchor and click sound | world-projected root/head midpoint plus 100 cm and 200 px canvas offset; click sound volume 1.25, pitch 1, start 0.2 | static HUD-adjacent anchor and no sound | Follow-up: BF-Char21 |

## PlayerStart, physics and ragdoll

| Item | ALS/UE source | Current result | Status |
| --- | --- | --- | --- |
| PlayerStart selection | GameMode prefers non-encroaching candidates, otherwise tries FindTeleportSpot Z/XY alternatives; pawn spawn uses PlayerStart yaw | current project admission supports one stable ray/shape fit and upward offsets only | Follow-up: BF-Char19 |
| Scene readiness | UE level initialization and GameMode spawning run after level actor initialization | current PlayerStart barrier exists, but manual has a separate sensor/gravity warmup adapter | Follow-up: BF-Char19 and BF-Char23 |
| PhysicsAsset topology | PA_Als has 21 bodies and 20 constrained links with body-specific colliders and limits | current asset has 16 bodies and 15 unconstrained spherical links | Follow-up: BF-Char22 |
| Ragdoll enter | source disables capsule collision, enables PhysicsBody, snapshots pelvis velocity and limits initial eight frames | current dynamic handoff/capsule disable works; speed cap and land threshold are absent | Follow-up: BF-Char22 |
| Ragdoll exit | grounded resumes Walking; airborne resumes Falling with pelvis velocity; facing selects get-up path | current restores target/layers/Tnua but clears velocity and has no facing/get-up state | Follow-up: BF-Char22 |
| Normal-state readback | source does not write ragdoll physics pose into normal mesh | normal animation pose restore guards base fallback readback | Corrected in BF-Char15 |

## Disposition

BF-Char16 owns the direct source-value corrections listed above and the complete audit record. BF-Char17 through BF-Char23 own every remaining implementation gap; none is treated as a completed parity claim. BF-Char99 may run only after those entries have their own source facts, focused runtime evidence and gates.
