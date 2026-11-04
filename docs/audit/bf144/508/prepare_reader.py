import hashlib
import json
import shutil
import sys
from pathlib import Path

mode = sys.argv[1]
assert mode in ('baseline', 'omit-write', 'restored')
root = Path(__file__).resolve().parent
source = Path('tools/ue_graph_source_reader')
copy = Path('tools/tmp/bf144/508/reader-source')
shutil.copytree(source, copy, dirs_exist_ok=True)
relative = Path('Source/AlsGraphSourceReader/Private/AlsGraphSourceReaderCommandlet.cpp')
original = (source / relative).read_bytes()
text = original.decode()
begin = text.index('TSharedPtr<FJsonObject> CaptureCharacterAnimationProbe(')
end = text.index('TSharedPtr<FJsonObject> CaptureRigNodeHistoryProbe(', begin)
function = text[begin:end]

old = '        const bool bOverrideFootIkBones = (*Case)->HasField(TEXT("initial_use_foot_ik_bones"));'
assert function.count(old) == 1
function = function.replace(old, '''        const bool bOverrideVelocityBlend = (*Case)->HasField(TEXT("velocity_blend_half_life"));
        double RequestedVelocityBlendHalfLife = 0;
        if (bOverrideVelocityBlend &&
            (!(*Case)->HasTypedField<EJson::Number>(TEXT("velocity_blend_half_life")) ||
                !(*Case)->TryGetNumberField(TEXT("velocity_blend_half_life"), RequestedVelocityBlendHalfLife) ||
                !FMath::IsFinite(static_cast<float>(RequestedVelocityBlendHalfLife)))) { return nullptr; }
''' + old)
old = '        const IAnimClassInterface* Interface = IAnimClassInterface::GetFromClass(Instance->GetClass());'
assert function.count(old) == 1
function = function.replace(old, '''        float ActualVelocityBlendHalfLife = 0;
        if (bOverrideVelocityBlend)
        {
            UObject* Original = SettingsProperty->GetObjectPropertyValue_InContainer(Instance);
            const FStructProperty* Grounded = FindFProperty<FStructProperty>(Original->GetClass(), TEXT("Grounded"));
            const FFloatProperty* HalfLife = Grounded != nullptr
                ? FindFProperty<FFloatProperty>(Grounded->Struct, TEXT("VelocityBlendInterpolationHalfLife")) : nullptr;
            if (HalfLife == nullptr) { return nullptr; }
            UObject* Copy = StaticDuplicateObject(Original, Instance);
            if (Copy == nullptr || Copy == Original) { return nullptr; }
            Copy->SetFlags(RF_Transient);
            void* Values = Grounded->ContainerPtrToValuePtr<void>(Copy);
            HalfLife->SetPropertyValue_InContainer(Values, static_cast<float>(RequestedVelocityBlendHalfLife));
            ActualVelocityBlendHalfLife = HalfLife->GetPropertyValue_InContainer(Values);
            SettingsProperty->SetObjectPropertyValue_InContainer(Instance, Copy);
        }
''' + old)
old = '        CaseRow->SetStringField(TEXT("name"), Name);'
assert function.count(old) == 1
function = function.replace(old, old + '''
        if (bOverrideVelocityBlend)
        {
            CaseRow->SetNumberField(TEXT("requested_velocity_blend_half_life"), RequestedVelocityBlendHalfLife);
            CaseRow->SetNumberField(TEXT("actual_velocity_blend_half_life"), ActualVelocityBlendHalfLife);
        }
''')
if mode == 'omit-write':
    line = '            HalfLife->SetPropertyValue_InContainer(Values, static_cast<float>(RequestedVelocityBlendHalfLife));\n'
    assert function.count(line) == 1
    function = function.replace(line, '')
body = (text[:begin] + function + text[end:]).encode()
(copy / relative).write_bytes(body)
assert (source / relative).read_bytes() == original
(root / f'reader-source-{mode}.json').write_text(json.dumps(dict(
    mode=mode, canonical_sha256=hashlib.sha256(original).hexdigest(),
    diagnostic_sha256=hashlib.sha256(body).hexdigest()), indent=2) + '\n')
print('Prepared isolated diagnostic reader ' + mode, flush=True)
