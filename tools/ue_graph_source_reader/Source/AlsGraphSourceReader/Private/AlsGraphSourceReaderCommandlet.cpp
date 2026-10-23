#include "AlsGraphSourceReaderCommandlet.h"

#include "Dom/JsonObject.h"
#include "EdGraph/EdGraph.h"
#include "EdGraph/EdGraphNode.h"
#include "EdGraph/EdGraphPin.h"
#include "AnimStateAliasNode.h"
#include "AnimStateNodeBase.h"
#include "AnimStateTransitionNode.h"
#include "AnimationRuntime.h"
#include "Math/ControlRigMathLibrary.h"
#include "AlphaBlend.h"
#include "Curves/CurveFloat.h"
#include "Animation/AnimInstanceProxy.h"
#include "Animation/AnimInstance.h"
#include "Animation/AnimSync.h"
#include "Components/SkeletalMeshComponent.h"
#include "Components/BoxComponent.h"
#include "Animation/AnimClassInterface.h"
#include "Animation/AnimBlueprintGeneratedClass.h"
#include "Animation/AnimNode_SaveCachedPose.h"
#include "Animation/AnimNode_Inertialization.h"
#include "Animation/AnimNode_ApplyMeshSpaceAdditive.h"
#include "Animation/AnimNode_SequencePlayer.h"
#include "Animation/AnimNode_Root.h"
#include "Animation/AnimNode_LinkedAnimGraph.h"
#include "Animation/AnimNode_StateMachine.h"
#include "Animation/AnimNode_StateResult.h"
#include "Animation/AnimNode_TransitionResult.h"
#include "Engine/World.h"
#include "AnimNodes/AnimNode_BlendListBase.h"
#include "AnimNodes/AnimNode_BlendSpacePlayer.h"
#include "AnimNodes/AnimNode_TwoWayBlend.h"
#include "AnimNodes/AnimNode_LayeredBoneBlend.h"
#include "AnimNodes/AnimNode_Slot.h"
#include "Animation/AnimMontageEvaluationState.h"
#include "Animation/AnimNotifyQueue.h"
#include "Animation/AnimNotifies/AnimNotifyState.h"
#include "Animation/AnimSequence.h"
#include "Animation/AnimRootMotionProvider.h"
#include "Animation/AnimBoneCompressionCodec.h"
#include "Animation/AnimCurveCompressionCodec.h"
#include "Animation/AnimSequenceDecompressionContext.h"
#include "Misc/Base64.h"
#include "Animation/AnimData/IAnimationDataModel.h"
#include "Sequencer/MovieSceneControlRigParameterSection.h"
#include "MovieScene.h"
#include "Animation/AnimationPoseData.h"
#include "Animation/AnimMontage.h"
#include "Animation/BlendSpace.h"
#include "Animation/BlendProfile.h"
#include "Animation/Skeleton.h"
#include "Engine/Blueprint.h"
#include "Engine/Engine.h"
#include "Engine/Font.h"
#include "Fonts/FontCache.h"
#include "Interfaces/ISlateNullRendererModule.h"
#include "Modules/ModuleManager.h"
#include "Engine/SkeletalMesh.h"
#include "Engine/StaticMesh.h"
#include "StaticMeshResources.h"
#include "Interface_CollisionDataProviderCore.h"
#include "Engine/Texture2D.h"
#include "Exporters/Exporter.h"
#include "ImageCore.h"
#include "ImageUtils.h"
#include "Components/PrimitiveComponent.h"
#include "PhysicsEngine/AggregateGeom.h"
#include "PhysicsEngine/PhysicsAsset.h"
#include "PhysicsEngine/PhysicsSettings.h"
#include "Chaos/MassConditioning.h"
#include "Chaos/Box.h"
#include "Chaos/Capsule.h"
#include "Chaos/Convex.h"
#include "Chaos/TriangleMeshImplicitObject.h"
#include "Chaos/ImplicitObjectScaled.h"
#include "Chaos/Sphere.h"
#include "Chaos/GeometryQueries.h"
#include "Chaos/ChaosEngineInterface.h"
#include "PBDRigidsSolver.h"
#include "ChaosSolversModule.h"
#include "Chaos/CollisionResolution.h"
#include "Chaos/Collision/CollisionConstraintAllocator.h"
#include "Chaos/Collision/CollisionFilter.h"
#include "Chaos/Collision/CapsuleTriangleContactPoint.h"
#include "Chaos/Collision/BasicCollisionDetector.h"
#include "PhysicsProxy/SingleParticlePhysicsProxy.h"
#include "PhysicsEngine/BodySetup.h"
#include "PhysicsEngine/PhysicsConstraintTemplate.h"
#include "PhysicsEngine/SkeletalBodySetup.h"
#include "PhysicalMaterials/PhysicalMaterial.h"
#include "Rigs/RigHierarchy.h"
#include "Rigs/FKControlRig.h"
#include "ControlRig.h"
#include "ControlRigObjectBinding.h"
#include "AnimNode_ControlRigBase.h"
#include "AnimNode_ControlRig.h"
#include "GameFramework/Actor.h"
#include "GameFramework/Character.h"
#include "GameFramework/CharacterMovementComponent.h"
#include "GameFramework/GameModeBase.h"
#include "GameFramework/PlayerController.h"
#include "GameFramework/WorldSettings.h"
#include "AssetCompilingManager.h"
#include "Misc/App.h"
#include "RigVMCore/RigVM.h"
#include "RigVMCore/RigVMProfilingInfo.h"
#include "RigVMModel/Nodes/RigVMUnitNode.h"
#include "HAL/FileManager.h"
#include "HAL/IConsoleManager.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "Misc/ScopeExit.h"
#include "Serialization/JsonSerializer.h"
#include "Serialization/JsonWriter.h"
#include "UObject/UObjectGlobals.h"
#include "UObject/UObjectHash.h"
#include "UObject/EnumProperty.h"
#include "UObject/UnrealType.h"
#include "UObject/StructOnScope.h"

DEFINE_LOG_CATEGORY_STATIC(LogAlsGraphSourceReader, Log, All);

namespace
{
FString ObjectPath(const UObject* Object)
{
    return IsValid(Object) ? Object->GetPathName() : TEXT("None");
}

FString GuidText(const FGuid& Guid)
{
    return Guid.IsValid() ? Guid.ToString(EGuidFormats::DigitsWithHyphensLower) : TEXT("None");
}

TSharedRef<FJsonObject> CaptureVector(const FVector& Vector)
{
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    Value->SetNumberField(TEXT("x"), Vector.X);
    Value->SetNumberField(TEXT("y"), Vector.Y);
    Value->SetNumberField(TEXT("z"), Vector.Z);
    return Value;
}

TSharedRef<FJsonObject> CaptureRotator(const FRotator& Rotator)
{
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    Value->SetNumberField(TEXT("pitch"), Rotator.Pitch);
    Value->SetNumberField(TEXT("yaw"), Rotator.Yaw);
    Value->SetNumberField(TEXT("roll"), Rotator.Roll);
    return Value;
}

TSharedRef<FJsonObject> CaptureQuaternion(const FQuat& Quaternion)
{
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    Value->SetNumberField(TEXT("x"), Quaternion.X);
    Value->SetNumberField(TEXT("y"), Quaternion.Y);
    Value->SetNumberField(TEXT("z"), Quaternion.Z);
    Value->SetNumberField(TEXT("w"), Quaternion.W);
    return Value;
}

TSharedRef<FJsonObject> CaptureTransform(const FTransform& Transform)
{
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    Value->SetObjectField(TEXT("translation"), CaptureVector(Transform.GetTranslation()));
    Value->SetObjectField(TEXT("rotation"), CaptureQuaternion(Transform.GetRotation()));
    Value->SetObjectField(TEXT("scale"), CaptureVector(Transform.GetScale3D()));
    return Value;
}

TArray<TSharedPtr<FJsonValue>> CaptureProperties(const UStruct* Struct, const void* Data, UObject* Parent, bool bIncludeUnitMetadata = false)
{
    TArray<TSharedPtr<FJsonValue>> Values;

    for (TFieldIterator<FProperty> Iterator(Struct, EFieldIterationFlags::IncludeSuper); Iterator; ++Iterator)
    {
        const FProperty* Property = *Iterator;
        for (int32 ArrayIndex = 0; ArrayIndex < Property->ArrayDim; ++ArrayIndex)
        {
            const void* Value = Property->ContainerPtrToValuePtr<void>(Data, ArrayIndex);
            FString ExportedText;
            Property->ExportTextItem_Direct(ExportedText, Value, nullptr, Parent, PPF_None);

            TSharedRef<FJsonObject> Entry = MakeShared<FJsonObject>();
            Entry->SetStringField(TEXT("name"), Property->GetName());
            Entry->SetNumberField(TEXT("array_index"), ArrayIndex);
            Entry->SetNumberField(TEXT("array_dim"), Property->ArrayDim);
            Entry->SetStringField(TEXT("property_class"), Property->GetClass()->GetName());
            Entry->SetStringField(TEXT("cpp_type"), Property->GetCPPType());
            Entry->SetStringField(TEXT("flags"), FString::Printf(TEXT("0x%llX"), Property->GetPropertyFlags()));
            Entry->SetStringField(TEXT("exported_text"), ExportedText);
            if (const FNumericProperty* Numeric = CastField<FNumericProperty>(Property); Numeric != nullptr && Numeric->IsFloatingPoint())
            {
                const double Number = Numeric->GetFloatingPointPropertyValue(Value);
                if (FMath::IsFinite(Number)) { Entry->SetNumberField(TEXT("numeric_value"), Number); }
            }
            if (const FEnumProperty* EnumProperty = CastField<FEnumProperty>(Property))
            {
                if (const FNumericProperty* Underlying = EnumProperty->GetUnderlyingProperty())
                {
                    const int64 EnumValue = Underlying->GetSignedIntPropertyValue(Value);
                    Entry->SetStringField(TEXT("enum_underlying_value"), FString::Printf(TEXT("%lld"), EnumValue));
                    if (const UEnum* Enum = EnumProperty->GetEnum())
                    {
                        Entry->SetStringField(TEXT("enum_value_name"), Enum->GetNameStringByValue(EnumValue));
                    }
                }
            }
            if (bIncludeUnitMetadata)
            {
                TSharedRef<FJsonObject> Metadata = MakeShared<FJsonObject>();
                for (const FName Key : {FName(TEXT("Input")), FName(TEXT("Output")), FName(TEXT("Constant")),
                    FName(TEXT("Singleton")), FName(TEXT("ForceUnits")), FName(TEXT("ClampMin")), FName(TEXT("ClampMax"))})
                {
                    if (Property->HasMetaData(Key))
                    {
                        Metadata->SetStringField(Key.ToString(), Property->GetMetaData(Key));
                    }
                }
                Entry->SetObjectField(TEXT("metadata"), Metadata);
                Entry->SetBoolField(TEXT("transient"), Property->HasAnyPropertyFlags(CPF_Transient));
            }
            if (const FStructProperty* StructProperty = CastField<FStructProperty>(Property))
            {
                Entry->SetArrayField(TEXT("fields"), CaptureProperties(StructProperty->Struct, Value, Parent, bIncludeUnitMetadata));
            }
            if (const FMapProperty* MapProperty = CastField<FMapProperty>(Property))
            {
                if (const FStructProperty* ValueStruct = CastField<FStructProperty>(MapProperty->ValueProp))
                {
                    FScriptMapHelper Map(MapProperty, Value);
                    TArray<TSharedPtr<FJsonValue>> Elements;
                    for (int32 Index = 0; Index < Map.GetMaxIndex(); ++Index)
                    {
                        if (!Map.IsValidIndex(Index)) { continue; }
                        TSharedRef<FJsonObject> Element = MakeShared<FJsonObject>();
                        FString Key;
                        MapProperty->KeyProp->ExportTextItem_Direct(Key, Map.GetKeyPtr(Index), nullptr, Parent, PPF_None);
                        Element->SetStringField(TEXT("key"), Key);
                        Element->SetArrayField(TEXT("fields"), CaptureProperties(ValueStruct->Struct, Map.GetValuePtr(Index), Parent, bIncludeUnitMetadata));
                        Elements.Add(MakeShared<FJsonValueObject>(Element));
                    }
                    Entry->SetArrayField(TEXT("elements"), Elements);
                }
            }
            Values.Add(MakeShared<FJsonValueObject>(Entry));
        }
    }

    return Values;
}

TSharedRef<FJsonObject> CaptureRigOperand(const FRigVMOperand& Operand)
{
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    Value->SetStringField(TEXT("memory"), StaticEnum<ERigVMMemoryType>()->GetNameStringByValue(static_cast<int64>(Operand.GetMemoryType())));
    Value->SetNumberField(TEXT("register"), Operand.GetRegisterIndex());
    Value->SetNumberField(TEXT("offset"), Operand.GetRegisterOffset());
    return Value;
}

FString RigPropertyCPPType(const FProperty* Property)
{
    FString ExtendedType;
    const FString BaseType = Property->GetCPPType(&ExtendedType);
    return BaseType + ExtendedType;
}

TSharedRef<FJsonObject> CaptureRigMemory(const FRigVMMemoryStorageStruct& Memory, UObject* Parent)
{
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    TArray<TSharedPtr<FJsonValue>> Properties;
    if (Memory.Num() > 0)
    {
        Properties = CaptureProperties(Memory.GetStruct(), Memory.GetValue().GetMemory(), Parent, true);
        for (const TSharedPtr<FJsonValue>& Row : Properties)
        {
            const TSharedPtr<FJsonObject> Entry = Row->AsObject();
            const int32 Index = Memory.GetPropertyIndexByName(FName(*Entry->GetStringField(TEXT("name"))));
            const FProperty* Property = Memory.GetProperty(Index);
            Entry->SetNumberField(TEXT("index"), Index);
            Entry->SetStringField(TEXT("cpp_type"), RigPropertyCPPType(Property));
            if (const FArrayProperty* ArrayProperty = CastField<FArrayProperty>(Property))
            {
                FScriptArrayHelper Array(ArrayProperty, Memory.GetData<uint8>(Index));
                TArray<TSharedPtr<FJsonValue>> Elements;
                for (int32 ElementIndex = 0; ElementIndex < Array.Num(); ++ElementIndex)
                {
                    TSharedRef<FJsonObject> Element = MakeShared<FJsonObject>();
                    FString Text;
                    ArrayProperty->Inner->ExportTextItem_Direct(Text, Array.GetRawPtr(ElementIndex), nullptr, Parent, PPF_None);
                    Element->SetStringField(TEXT("exported_text"), Text);
                    if (const FNumericProperty* Numeric = CastField<FNumericProperty>(ArrayProperty->Inner); Numeric != nullptr && Numeric->IsFloatingPoint())
                    {
                        const double Number = Numeric->GetFloatingPointPropertyValue(Array.GetRawPtr(ElementIndex));
                        if (FMath::IsFinite(Number)) { Element->SetNumberField(TEXT("numeric_value"), Number); }
                    }
                    if (const FStructProperty* InnerStruct = CastField<FStructProperty>(ArrayProperty->Inner))
                    {
                        Element->SetArrayField(TEXT("fields"), CaptureProperties(InnerStruct->Struct, Array.GetRawPtr(ElementIndex), Parent, true));
                    }
                    Elements.Add(MakeShared<FJsonValueObject>(Element));
                }
                Entry->SetArrayField(TEXT("elements"), Elements);
            }
        }
    }
    Value->SetArrayField(TEXT("properties"), Properties);
    TArray<TSharedPtr<FJsonValue>> Paths;
    for (const FRigVMPropertyPath& Path : Memory.GetPropertyPaths())
    {
        TSharedRef<FJsonObject> Entry = MakeShared<FJsonObject>();
        Entry->SetStringField(TEXT("path"), Path.ToString());
        Entry->SetStringField(TEXT("head_cpp_type"), Path.GetHeadCPPType());
        Entry->SetStringField(TEXT("tail_cpp_type"), RigPropertyCPPType(Path.GetTailProperty()));
        Paths.Add(MakeShared<FJsonValueObject>(Entry));
    }
    Value->SetArrayField(TEXT("paths"), Paths);
    return Value;
}

TSharedRef<FJsonObject> CaptureRigVM(const URigVM& VM, UObject* Parent)
{
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    const FRigVMByteCode& Code = VM.GetByteCode();
    const FRigVMInstructionArray Instructions = Code.GetInstructions();
    bool bComplete = Instructions.Num() > 0 && !VM.IsNativized() && Code.NumCallables() == 0 && Code.NumPredicateBranches() == 0;
    Value->SetStringField(TEXT("object_path"), VM.GetPathName());
    Value->SetObjectField(TEXT("literal"), CaptureRigMemory(VM.GetDefaultLiteralMemory(), Parent));
    Value->SetObjectField(TEXT("work"), CaptureRigMemory(VM.GetDefaultWorkMemory(), Parent));
    Value->SetNumberField(TEXT("callable_count"), Code.NumCallables());
    Value->SetNumberField(TEXT("predicate_count"), Code.NumPredicateBranches());
    TArray<TSharedPtr<FJsonValue>> Functions;
    for (int32 Index = 0; Index < VM.GetFunctionNames().Num(); ++Index)
    {
        TSharedRef<FJsonObject> Entry = MakeShared<FJsonObject>();
        Entry->SetStringField(TEXT("name"), VM.GetFunctionNames()[Index].ToString());
        const FRigVMFunction* Function = VM.GetFunctions().IsValidIndex(Index) ? VM.GetFunctions()[Index] : nullptr;
        TArray<TSharedPtr<FJsonValue>> Arguments;
        if (Function != nullptr)
        {
            for (const FRigVMFunctionArgument& Argument : Function->GetArguments())
            {
                TSharedRef<FJsonObject> ArgumentValue = MakeShared<FJsonObject>();
                ArgumentValue->SetStringField(TEXT("name"), Argument.Name);
                ArgumentValue->SetStringField(TEXT("cpp_type"), Argument.Type);
                Arguments.Add(MakeShared<FJsonValueObject>(ArgumentValue));
            }
        }
        else
        {
            bComplete = false;
        }
        Entry->SetArrayField(TEXT("arguments"), Arguments);
        Functions.Add(MakeShared<FJsonValueObject>(Entry));
    }
    Value->SetArrayField(TEXT("functions"), Functions);
    TArray<TSharedPtr<FJsonValue>> ExternalVariables;
    for (const FRigVMExternalVariableDef& Variable : VM.GetExternalVariableDefs())
    {
        TSharedRef<FJsonObject> Entry = MakeShared<FJsonObject>();
        Entry->SetStringField(TEXT("name"), Variable.GetName().ToString());
        Entry->SetStringField(TEXT("cpp_type"), Variable.GetExtendedCPPType().ToString());
        Entry->SetStringField(TEXT("type_object"), ObjectPath(Variable.GetCPPTypeObject()));
        ExternalVariables.Add(MakeShared<FJsonValueObject>(Entry));
    }
    Value->SetArrayField(TEXT("external_variables"), ExternalVariables);
    TArray<TSharedPtr<FJsonValue>> ExternalPaths;
    for (const FRigVMPropertyPathDescription& Path : VM.ExternalPropertyPathDescriptions)
    {
        TSharedRef<FJsonObject> Entry = MakeShared<FJsonObject>();
        Entry->SetNumberField(TEXT("register"), Path.PropertyIndex);
        Entry->SetStringField(TEXT("head_cpp_type"), Path.HeadCPPType);
        Entry->SetStringField(TEXT("path"), Path.SegmentPath);
        const FRigVMPropertyPath Resolved(VM.GetExternalVariableDefs()[Path.PropertyIndex].GetProperty(), Path.SegmentPath);
        Entry->SetStringField(TEXT("tail_cpp_type"), RigPropertyCPPType(Resolved.GetTailProperty()));
        ExternalPaths.Add(MakeShared<FJsonValueObject>(Entry));
    }
    Value->SetArrayField(TEXT("external_paths"), ExternalPaths);
    TArray<TSharedPtr<FJsonValue>> Entries;
    for (int32 Index = 0; Index < Code.NumEntries(); ++Index)
    {
        const FRigVMByteCodeEntry& Entry = Code.GetEntry(Index);
        TSharedRef<FJsonObject> EntryValue = MakeShared<FJsonObject>();
        EntryValue->SetStringField(TEXT("name"), Entry.Name.ToString());
        EntryValue->SetNumberField(TEXT("instruction"), Entry.InstructionIndex);
        Entries.Add(MakeShared<FJsonValueObject>(EntryValue));
    }
    Value->SetArrayField(TEXT("entries"), Entries);
    TArray<TSharedPtr<FJsonValue>> Branches;
    for (int32 Index = 0; Index < Code.NumBranches(); ++Index)
    {
        const FRigVMBranchInfo& Branch = Code.GetBranch(Index);
        TSharedRef<FJsonObject> Entry = MakeShared<FJsonObject>();
        Entry->SetStringField(TEXT("label"), Branch.Label.ToString());
        Entry->SetNumberField(TEXT("instruction"), Branch.InstructionIndex);
        Entry->SetNumberField(TEXT("argument"), Branch.ArgumentIndex);
        Entry->SetNumberField(TEXT("first"), Branch.FirstInstruction);
        Entry->SetNumberField(TEXT("last"), Branch.LastInstruction);
        Branches.Add(MakeShared<FJsonValueObject>(Entry));
    }
    Value->SetArrayField(TEXT("branches"), Branches);
    TArray<TSharedPtr<FJsonValue>> Ops;
    for (const FRigVMInstruction& Instruction : Instructions)
    {
        TSharedRef<FJsonObject> Entry = MakeShared<FJsonObject>();
        Entry->SetNumberField(TEXT("index"), Instruction.Index);
        Entry->SetStringField(TEXT("opcode"), StaticEnum<ERigVMOpCode>()->GetNameStringByValue(static_cast<int64>(Instruction.OpCode)));
        Entry->SetStringField(TEXT("call_path"), Code.GetCallPathForInstruction(Instruction.Index));
        Entry->SetStringField(TEXT("subject"), ObjectPath(Code.GetSubjectForInstruction(Instruction.Index)));
        switch (Instruction.OpCode)
        {
        case ERigVMOpCode::Execute:
        {
            const FRigVMExecuteOp& Op = Code.GetOpAt<FRigVMExecuteOp>(Instruction);
            Entry->SetNumberField(TEXT("function"), Op.CallableIndex);
            Entry->SetNumberField(TEXT("predicate_count"), Op.PredicateCount);
            TArray<TSharedPtr<FJsonValue>> Operands;
            for (const FRigVMOperand& Operand : Code.GetOperandsForCallableOp(Instruction))
            {
                Operands.Add(MakeShared<FJsonValueObject>(CaptureRigOperand(Operand)));
            }
            Entry->SetArrayField(TEXT("operands"), Operands);
            break;
        }
        case ERigVMOpCode::Copy:
        {
            const FRigVMCopyOp& Op = Code.GetOpAt<FRigVMCopyOp>(Instruction);
            Entry->SetObjectField(TEXT("source"), CaptureRigOperand(Op.Source));
            Entry->SetObjectField(TEXT("target"), CaptureRigOperand(Op.Target));
            break;
        }
        case ERigVMOpCode::Zero:
            Entry->SetObjectField(TEXT("target"), CaptureRigOperand(Code.GetOpAt<FRigVMUnaryOp>(Instruction).Arg));
            break;
        case ERigVMOpCode::JumpAbsolute:
        case ERigVMOpCode::JumpForward:
        case ERigVMOpCode::JumpBackward:
            Entry->SetNumberField(TEXT("jump_offset"), Code.GetOpAt<FRigVMJumpOp>(Instruction).InstructionIndex);
            break;
        case ERigVMOpCode::JumpToBranch:
        {
            const FRigVMJumpToBranchOp& Op = Code.GetOpAt<FRigVMJumpToBranchOp>(Instruction);
            Entry->SetObjectField(TEXT("selector"), CaptureRigOperand(Op.Arg));
            Entry->SetNumberField(TEXT("first_branch"), Op.FirstBranchInfoIndex);
            break;
        }
        case ERigVMOpCode::Exit:
            break;
        default:
            bComplete = false;
            break;
        }
        Ops.Add(MakeShared<FJsonValueObject>(Entry));
    }
    Value->SetArrayField(TEXT("instructions"), Ops);
    Value->SetBoolField(TEXT("capture_complete"), bComplete);
    return Value;
}

TSharedRef<FJsonObject> CapturePinReference(const UEdGraphPin* Pin)
{
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    const UEdGraphNode* Node = Pin != nullptr ? Pin->GetOwningNodeUnchecked() : nullptr;
    Value->SetStringField(TEXT("node_object_path"), ObjectPath(Node));
    Value->SetStringField(TEXT("node_guid"), Node != nullptr ? GuidText(Node->NodeGuid) : TEXT("None"));
    Value->SetStringField(TEXT("pin_id"), Pin != nullptr ? GuidText(Pin->PinId) : TEXT("None"));
    Value->SetStringField(TEXT("pin_name"), Pin != nullptr ? Pin->PinName.ToString() : TEXT("None"));
    Value->SetStringField(
        TEXT("direction"),
        Pin == nullptr ? TEXT("None") : (Pin->Direction == EGPD_Input ? TEXT("Input") : TEXT("Output"))
    );
    return Value;
}

TSharedRef<FJsonObject> CapturePin(const UEdGraphPin* Pin, int32 Index)
{
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    Value->SetNumberField(TEXT("index"), Index);
    Value->SetStringField(TEXT("pin_id"), GuidText(Pin->PinId));
    Value->SetStringField(TEXT("persistent_guid"), GuidText(Pin->PersistentGuid));
    Value->SetStringField(TEXT("name"), Pin->PinName.ToString());
    Value->SetNumberField(TEXT("source_index"), Pin->SourceIndex);
    Value->SetStringField(TEXT("direction"), Pin->Direction == EGPD_Input ? TEXT("Input") : TEXT("Output"));
    Value->SetBoolField(TEXT("hidden"), Pin->bHidden != 0);
    Value->SetBoolField(TEXT("not_connectable"), Pin->bNotConnectable != 0);
    Value->SetBoolField(TEXT("default_value_read_only"), Pin->bDefaultValueIsReadOnly != 0);
    Value->SetBoolField(TEXT("default_value_ignored"), Pin->bDefaultValueIsIgnored != 0);
    Value->SetBoolField(TEXT("advanced_view"), Pin->bAdvancedView != 0);
    Value->SetBoolField(TEXT("deprecated"), Pin->bDeprecated != 0);
    Value->SetBoolField(TEXT("display_as_mutable_ref"), Pin->bDisplayAsMutableRef != 0);
    Value->SetBoolField(TEXT("allow_friendly_name"), Pin->bAllowFriendlyName != 0);
    Value->SetBoolField(TEXT("orphaned"), Pin->bOrphanedPin != 0);
    Value->SetBoolField(TEXT("backwards_compat_empty_default"), Pin->bUseBackwardsCompatForEmptyAutogeneratedValue != 0);
    Value->SetStringField(TEXT("friendly_name"), Pin->PinFriendlyName.ToString());
    Value->SetStringField(TEXT("deprecation_message"), Pin->DeprecationMessage);
    Value->SetStringField(TEXT("tooltip"), Pin->PinToolTip);
    Value->SetStringField(TEXT("default_value"), Pin->DefaultValue);
    Value->SetStringField(TEXT("autogenerated_default_value"), Pin->AutogeneratedDefaultValue);
    Value->SetStringField(TEXT("default_object"), ObjectPath(Pin->DefaultObject));
    Value->SetStringField(TEXT("default_text_value"), Pin->DefaultTextValue.ToString());
    Value->SetArrayField(TEXT("pin_type_properties"), CaptureProperties(FEdGraphPinType::StaticStruct(), &Pin->PinType, Pin->GetOwningNodeUnchecked()));

    TArray<TSharedPtr<FJsonValue>> Links;
    for (const UEdGraphPin* Linked : Pin->LinkedTo)
    {
        Links.Add(MakeShared<FJsonValueObject>(CapturePinReference(Linked)));
    }
    Value->SetArrayField(TEXT("linked_to"), Links);

    TArray<TSharedPtr<FJsonValue>> SubPins;
    for (const UEdGraphPin* SubPin : Pin->SubPins)
    {
        SubPins.Add(MakeShared<FJsonValueObject>(CapturePinReference(SubPin)));
    }
    Value->SetArrayField(TEXT("sub_pins"), SubPins);
    Value->SetObjectField(TEXT("parent_pin"), CapturePinReference(Pin->ParentPin));
    Value->SetObjectField(TEXT("reference_pass_through_connection"), CapturePinReference(Pin->ReferencePassThroughConnection));
    return Value;
}

TSharedRef<FJsonObject> CaptureNode(const UEdGraphNode* Node, int32 Index)
{
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    Value->SetNumberField(TEXT("index"), Index);
    Value->SetStringField(TEXT("object_path"), ObjectPath(Node));
    Value->SetStringField(TEXT("class_path"), ObjectPath(Node->GetClass()));
    Value->SetStringField(TEXT("node_guid"), GuidText(Node->NodeGuid));
    Value->SetStringField(TEXT("title"), Node->GetNodeTitle(ENodeTitleType::FullTitle).ToString());
    Value->SetNumberField(TEXT("position_x"), Node->NodePosX);
    Value->SetNumberField(TEXT("position_y"), Node->NodePosY);
    Value->SetStringField(TEXT("comment"), Node->NodeComment);
    Value->SetBoolField(TEXT("enabled"), Node->IsNodeEnabled());
    Value->SetArrayField(TEXT("properties"), CaptureProperties(Node->GetClass(), Node, const_cast<UEdGraphNode*>(Node)));

    TArray<TSharedPtr<FJsonValue>> Pins;
    for (int32 PinIndex = 0; PinIndex < Node->Pins.Num(); ++PinIndex)
    {
        const UEdGraphPin* Pin = Node->Pins[PinIndex];
        if (Pin != nullptr)
        {
            Pins.Add(MakeShared<FJsonValueObject>(CapturePin(Pin, PinIndex)));
        }
    }
    Value->SetArrayField(TEXT("pins"), Pins);
    UAnimStateNodeBase* State = Cast<UAnimStateNodeBase>(const_cast<UEdGraphNode*>(Node));
    if (State != nullptr && !State->IsA<UAnimStateAliasNode>() && !State->IsA<UAnimStateTransitionNode>() && State->Pins.Num() >= 2)
    {
        TMultiMap<UAnimStateNodeBase*, UAnimStateAliasNode*> AliasMap;
        for (UEdGraphNode* Peer : State->GetGraph()->Nodes)
        {
            UAnimStateAliasNode* Alias = Cast<UAnimStateAliasNode>(Peer);
            if (Alias != nullptr && (Alias->bGlobalAlias || Alias->GetAliasedStates().Contains(State)))
            {
                AliasMap.Add(State, Alias);
            }
        }
        TArray<UAnimStateAliasNode*> Aliases;
        AliasMap.MultiFind(State, Aliases);
        TArray<UAnimStateTransitionNode*> Transitions;
        for (const UAnimStateAliasNode* Alias : Aliases)
        {
            Alias->GetTransitionList(Transitions, false);
        }
        State->GetTransitionList(Transitions, true);
        TArray<TSharedPtr<FJsonValue>> Order;
        for (const UAnimStateTransitionNode* Transition : Transitions)
        {
            if (!Transition->bDisabled)
            {
                Order.Add(MakeShared<FJsonValueString>(ObjectPath(Transition)));
            }
        }
        Value->SetArrayField(TEXT("runtime_transition_order"), Order);
    }
    return Value;
}

TSharedRef<FJsonObject> CaptureObject(const UObject* Object, int32 Index)
{
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    Value->SetNumberField(TEXT("index"), Index);
    Value->SetStringField(TEXT("object_path"), ObjectPath(Object));
    Value->SetStringField(TEXT("class_path"), ObjectPath(Object->GetClass()));
    Value->SetArrayField(TEXT("properties"), CaptureProperties(Object->GetClass(), Object, const_cast<UObject*>(Object)));
    if (const UPrimitiveComponent* Primitive = Cast<UPrimitiveComponent>(Object))
    {
        TArray<TSharedPtr<FJsonValue>> BlockingChannels;
        if (Primitive->IsQueryCollisionEnabled())
        {
            for (int32 Channel = 0; Channel < ECC_OverlapAll_Deprecated; ++Channel)
            {
                if (Primitive->GetCollisionResponseToChannel(static_cast<ECollisionChannel>(Channel)) == ECR_Block)
                {
                    BlockingChannels.Add(MakeShared<FJsonValueString>(
                        StaticEnum<ECollisionChannel>()->GetNameStringByValue(Channel).RightChop(4)));
                }
            }
        }
        Value->SetArrayField(TEXT("blocking_query_channels"), BlockingChannels);
    }
    return Value;
}

TSharedRef<FJsonObject> CaptureGraph(const UEdGraph* Graph, int32 Index)
{
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    Value->SetNumberField(TEXT("index"), Index);
    Value->SetStringField(TEXT("object_path"), ObjectPath(Graph));
    Value->SetStringField(TEXT("class_path"), ObjectPath(Graph->GetClass()));
    Value->SetStringField(TEXT("name"), Graph->GetName());
    Value->SetArrayField(TEXT("properties"), CaptureProperties(Graph->GetClass(), Graph, const_cast<UEdGraph*>(Graph)));

    TArray<TSharedPtr<FJsonValue>> Nodes;
    for (int32 NodeIndex = 0; NodeIndex < Graph->Nodes.Num(); ++NodeIndex)
    {
        const UEdGraphNode* Node = Graph->Nodes[NodeIndex];
        if (Node != nullptr)
        {
            Nodes.Add(MakeShared<FJsonValueObject>(CaptureNode(Node, NodeIndex)));
        }
    }
    Value->SetArrayField(TEXT("nodes"), Nodes);
    return Value;
}

TSharedRef<FJsonObject> CapturePhysicsBody(const USkeletalBodySetup* Body, int32 Index)
{
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    Value->SetNumberField(TEXT("index"), Index);
    Value->SetStringField(TEXT("bone_name"), Body->BoneName.ToString());
    Value->SetNumberField(TEXT("physics_type"), static_cast<int32>(Body->PhysicsType.GetValue()));
    Value->SetNumberField(TEXT("collision_response"), static_cast<int32>(Body->CollisionReponse.GetValue()));
    Value->SetNumberField(TEXT("calculated_mass"), Body->CalculateMass());
    Value->SetNumberField(TEXT("sleep_threshold_multiplier"), Body->DefaultInstance.GetSleepThresholdMultiplier());
    Value->SetNumberField(TEXT("shape_count"), Body->AggGeom.GetElementCount());
    if (UPhysicalMaterial* Material = FBodyInstance::GetSimplePhysicalMaterial(
        &Body->DefaultInstance, nullptr, const_cast<USkeletalBodySetup*>(Body)))
    {
        Value->SetStringField(TEXT("physical_material_path"), ObjectPath(Material));
        Value->SetArrayField(TEXT("physical_material_properties"), CaptureProperties(Material->GetClass(), Material, Material));
    }
    Value->SetArrayField(
        TEXT("default_instance_properties"),
        CaptureProperties(FBodyInstance::StaticStruct(), &Body->DefaultInstance, const_cast<USkeletalBodySetup*>(Body))
    );

    TArray<TSharedPtr<FJsonValue>> Shapes;
    for (int32 ShapeIndex = 0; ShapeIndex < Body->AggGeom.SphereElems.Num(); ++ShapeIndex)
    {
        const FKSphereElem& Shape = Body->AggGeom.SphereElems[ShapeIndex];
        TSharedRef<FJsonObject> ShapeValue = MakeShared<FJsonObject>();
        ShapeValue->SetNumberField(TEXT("index"), ShapeIndex);
        ShapeValue->SetStringField(TEXT("kind"), TEXT("sphere"));
        ShapeValue->SetObjectField(TEXT("center"), CaptureVector(Shape.Center));
        ShapeValue->SetNumberField(TEXT("radius"), Shape.Radius);
        ShapeValue->SetArrayField(TEXT("properties"), CaptureProperties(FKSphereElem::StaticStruct(), &Shape, const_cast<USkeletalBodySetup*>(Body)));
        Shapes.Add(MakeShared<FJsonValueObject>(ShapeValue));
    }
    for (int32 ShapeIndex = 0; ShapeIndex < Body->AggGeom.BoxElems.Num(); ++ShapeIndex)
    {
        const FKBoxElem& Shape = Body->AggGeom.BoxElems[ShapeIndex];
        TSharedRef<FJsonObject> ShapeValue = MakeShared<FJsonObject>();
        ShapeValue->SetNumberField(TEXT("index"), ShapeIndex);
        ShapeValue->SetStringField(TEXT("kind"), TEXT("box"));
        ShapeValue->SetObjectField(TEXT("center"), CaptureVector(Shape.Center));
        ShapeValue->SetObjectField(TEXT("rotation"), CaptureRotator(Shape.Rotation));
        ShapeValue->SetNumberField(TEXT("x"), Shape.X);
        ShapeValue->SetNumberField(TEXT("y"), Shape.Y);
        ShapeValue->SetNumberField(TEXT("z"), Shape.Z);
        ShapeValue->SetArrayField(TEXT("properties"), CaptureProperties(FKBoxElem::StaticStruct(), &Shape, const_cast<USkeletalBodySetup*>(Body)));
        Shapes.Add(MakeShared<FJsonValueObject>(ShapeValue));
    }
    for (int32 ShapeIndex = 0; ShapeIndex < Body->AggGeom.SphylElems.Num(); ++ShapeIndex)
    {
        const FKSphylElem& Shape = Body->AggGeom.SphylElems[ShapeIndex];
        TSharedRef<FJsonObject> ShapeValue = MakeShared<FJsonObject>();
        ShapeValue->SetNumberField(TEXT("index"), ShapeIndex);
        ShapeValue->SetStringField(TEXT("kind"), TEXT("capsule"));
        ShapeValue->SetObjectField(TEXT("center"), CaptureVector(Shape.Center));
        ShapeValue->SetObjectField(TEXT("rotation"), CaptureRotator(Shape.Rotation));
        ShapeValue->SetNumberField(TEXT("radius"), Shape.Radius);
        ShapeValue->SetNumberField(TEXT("length"), Shape.Length);
        ShapeValue->SetArrayField(TEXT("properties"), CaptureProperties(FKSphylElem::StaticStruct(), &Shape, const_cast<USkeletalBodySetup*>(Body)));
        Shapes.Add(MakeShared<FJsonValueObject>(ShapeValue));
    }
    Value->SetArrayField(TEXT("shapes"), Shapes);
    return Value;
}

TSharedRef<FJsonObject> CapturePhysicsConstraint(const UPhysicsConstraintTemplate* Constraint, int32 Index)
{
    const FConstraintInstance& Instance = Constraint->DefaultInstance;
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    Value->SetNumberField(TEXT("index"), Index);
    Value->SetStringField(TEXT("joint_name"), Instance.JointName.ToString());
    Value->SetStringField(TEXT("constraint_bone_1"), Instance.ConstraintBone1.ToString());
    Value->SetStringField(TEXT("constraint_bone_2"), Instance.ConstraintBone2.ToString());
    Value->SetObjectField(TEXT("position_1"), CaptureVector(Instance.Pos1));
    Value->SetObjectField(TEXT("primary_axis_1"), CaptureVector(Instance.PriAxis1));
    Value->SetObjectField(TEXT("secondary_axis_1"), CaptureVector(Instance.SecAxis1));
    Value->SetObjectField(TEXT("position_2"), CaptureVector(Instance.Pos2));
    Value->SetObjectField(TEXT("primary_axis_2"), CaptureVector(Instance.PriAxis2));
    Value->SetObjectField(TEXT("secondary_axis_2"), CaptureVector(Instance.SecAxis2));
    Value->SetObjectField(TEXT("angular_rotation_offset"), CaptureRotator(Instance.AngularRotationOffset));
    Value->SetArrayField(
        TEXT("instance_properties"),
        CaptureProperties(FConstraintInstance::StaticStruct(), &Instance, const_cast<UPhysicsConstraintTemplate*>(Constraint))
    );
    return Value;
}

TSharedRef<FJsonObject> CapturePhysicsAsset(const UPhysicsAsset* Asset)
{
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    Value->SetNumberField(TEXT("project_default_dof_mode"), static_cast<int32>(UPhysicsSettings::Get()->DefaultDegreesOfFreedom.GetValue()));
    Value->SetNumberField(TEXT("project_initial_overlap_depenetration_velocity_cm_s"), UPhysicsSettings::Get()->SolverOptions.CollisionInitialOverlapDepenetrationVelocity);
    TSharedRef<FJsonObject> ConsoleVariables = MakeShared<FJsonObject>();
    const FConsoleObjectVisitor Visitor = FConsoleObjectVisitor::CreateLambda([&ConsoleVariables](const TCHAR* Name, IConsoleObject* Object)
    {
        if (IConsoleVariable* Variable = Object->AsVariable())
        {
            ConsoleVariables->SetStringField(Name, Variable->GetString());
        }
    });
    for (const TCHAR* Prefix : {TEXT("p.Chaos.JointConstraint."), TEXT("p.Chaos.Solver.Joint."), TEXT("p.Chaos.Joint."), TEXT("p.Chaos.CCD."), TEXT("p.Chaos.Solver.InertiaConditioning."), TEXT("p.Chaos.InertiaConditioning."), TEXT("p.Chaos.ExcludeFreeJointForInertiaConditioning"), TEXT("p.Chaos.Solver.Iterations."), TEXT("p.Chaos.Solver.ComputeIterationSettings."), TEXT("p.AngularEtherDragOverride"), TEXT("p.HackMaxAngularVelocity"), TEXT("p.LinearEtherDragOverride"), TEXT("p.HackMaxVelocity2"), TEXT("p.Chaos.Solver.Sleep."), TEXT("p.Chaos.SmoothedPositionLerpRate"), TEXT("p.Chaos.Solver.Collision."), TEXT("p.CollisionRestitution"), TEXT("p.Chaos.PBDCollisionSolver."), TEXT("p.Chaos.Solver.bChaosForceMACD"), TEXT("p.Chaos.Collision.Stiffness"), TEXT("p.CollisionFriction"), TEXT("p.CollisionAngularFriction"), TEXT("p.CollisionBaseFrictionImpulse"), TEXT("p.Chaos.Solver.Use1DFriction"), TEXT("p.Chaos.Collision.Manifold.Friction"), TEXT("p.Chaos.Collision.Manifold.EnableFrictionRestore"), TEXT("p.Chaos.Collision.Manifold.EnableGjkWarmStart"), TEXT("p.Chaos.Collision.SimpleAssignContact"), TEXT("p.Chaos.Collision.UseCapsuleTriMesh2"), TEXT("p.Chaos.Collision.ConvexTriMesh"), TEXT("p.Chaos.Collision.EnableMeshManifoldOptimizedLoop"), TEXT("p.Chaos.Collision.MeshContact"), TEXT("p.Chaos.MeshContactGenerator."), TEXT("p.Chaos.Collision.UseGJK2"), TEXT("p.Chaos.Collision.GJKEpsilon"), TEXT("p.Chaos.Collision.EPAEpsilon")})
    {
        IConsoleManager::Get().ForEachConsoleObjectThatStartsWith(Visitor, Prefix);
    }
    Value->SetObjectField(TEXT("constraint_console_variables"), ConsoleVariables);
    Chaos::FInertiaConditioningTolerances ConditioningTolerances;
    ConditioningTolerances.InvMassTolerance = FCString::Atof(*ConsoleVariables->GetStringField(TEXT("p.Chaos.InertiaConditioning.InvMassTolerance")));
    ConditioningTolerances.InvInertiaTolerance = FCString::Atof(*ConsoleVariables->GetStringField(TEXT("p.Chaos.InertiaConditioning.InvInertiaTolerance")));
    ConditioningTolerances.ExtentTolerance = FCString::Atof(*ConsoleVariables->GetStringField(TEXT("p.Chaos.InertiaConditioning.ExtentTolerance")));
    const float ConditioningDistance = FCString::Atof(*ConsoleVariables->GetStringField(TEXT("p.Chaos.Solver.InertiaConditioning.Distance")));
    const float ConditioningRatio = FCString::Atof(*ConsoleVariables->GetStringField(TEXT("p.Chaos.Solver.InertiaConditioning.RotationRatio")));
    TArray<TSharedPtr<FJsonValue>> ConditioningFixtures;
    for (const float InvMass : {0.f, 1.f, 10.f})
    for (const Chaos::FVec3f InvInertia : {Chaos::FVec3f(0.06f), Chaos::FVec3f(0.04f, 0.02f, 0.01f), Chaos::FVec3f(0.0001f, 0.0002f, 0.0003f), Chaos::FVec3f(3.f, 30.f, 300.f)})
    for (const Chaos::FVec3f Extents : {Chaos::FVec3f(5.f), Chaos::FVec3f(0.5f, 10.f, 0.25f), Chaos::FVec3f(0.f, 5.f, 5.f), Chaos::FVec3f(20.f, 10.f, 50.f)})
    for (const float MaxDistance : {0.f, ConditioningDistance})
    for (const float MaxComponentRatio : {0.f, 2.f})
    {
        TSharedRef<FJsonObject> Fixture = MakeShared<FJsonObject>();
        Fixture->SetNumberField(TEXT("inverse_mass"), InvMass);
        Fixture->SetObjectField(TEXT("inverse_inertia"), CaptureVector(FVector(InvInertia)));
        Fixture->SetObjectField(TEXT("extents"), CaptureVector(FVector(Extents)));
        Fixture->SetNumberField(TEXT("maximum_distance"), MaxDistance);
        Fixture->SetNumberField(TEXT("maximum_rotation_ratio"), ConditioningRatio);
        Fixture->SetNumberField(TEXT("maximum_component_ratio"), MaxComponentRatio);
        const Chaos::FVec3f Scale = Chaos::CalculateInertiaConditioning(InvMass, InvInertia, Extents, MaxDistance, ConditioningRatio, MaxComponentRatio, ConditioningTolerances);
        Fixture->SetObjectField(TEXT("inverse_scale"), CaptureVector(FVector(Scale)));
        ConditioningFixtures.Add(MakeShared<FJsonValueObject>(Fixture));
    }
    Value->SetArrayField(TEXT("inertia_conditioning_fixtures"), ConditioningFixtures);
    Value->SetArrayField(
        TEXT("solver_settings_properties"),
        CaptureProperties(FPhysicsAssetSolverSettings::StaticStruct(), &Asset->SolverSettings, const_cast<UPhysicsAsset*>(Asset))
    );

    TArray<TSharedPtr<FJsonValue>> Bodies;
    for (int32 BodyIndex = 0; BodyIndex < Asset->SkeletalBodySetups.Num(); ++BodyIndex)
    {
        if (const USkeletalBodySetup* Body = Asset->SkeletalBodySetups[BodyIndex])
        {
            Bodies.Add(MakeShared<FJsonValueObject>(CapturePhysicsBody(Body, BodyIndex)));
        }
    }
    Value->SetArrayField(TEXT("bodies"), Bodies);

    TArray<TSharedPtr<FJsonValue>> Constraints;
    for (int32 ConstraintIndex = 0; ConstraintIndex < Asset->ConstraintSetup.Num(); ++ConstraintIndex)
    {
        if (const UPhysicsConstraintTemplate* Constraint = Asset->ConstraintSetup[ConstraintIndex])
        {
            Constraints.Add(MakeShared<FJsonValueObject>(CapturePhysicsConstraint(Constraint, ConstraintIndex)));
        }
    }
    Value->SetArrayField(TEXT("constraints"), Constraints);

    TArray<TSharedPtr<FJsonValue>> CollisionDisabledPairs;
    for (const TPair<FRigidBodyIndexPair, bool>& Pair : Asset->CollisionDisableTable)
    {
        TSharedRef<FJsonObject> PairValue = MakeShared<FJsonObject>();
        PairValue->SetNumberField(TEXT("body_index_1"), Pair.Key.Indices[0]);
        PairValue->SetNumberField(TEXT("body_index_2"), Pair.Key.Indices[1]);
        PairValue->SetBoolField(TEXT("stored_value"), Pair.Value);
        PairValue->SetBoolField(TEXT("disabled"), !Asset->IsCollisionEnabled(Pair.Key.Indices[0], Pair.Key.Indices[1]));
        CollisionDisabledPairs.Add(MakeShared<FJsonValueObject>(PairValue));
    }
    Value->SetArrayField(TEXT("collision_disabled_pairs"), CollisionDisabledPairs);
    return Value;
}

TSharedRef<FJsonObject> CapturePhysicsMaterialProbe()
{
    UPhysicalMaterial* Shared = NewObject<UPhysicalMaterial>(GetTransientPackage());
    Shared->Friction = 0.2f;
    Shared->Restitution = 0.1f;
    UPhysicalMaterial* Override = NewObject<UPhysicalMaterial>(GetTransientPackage());
    Override->Friction = 0.8f;
    Override->Restitution = 0.6f;
    USkeletalBodySetup* Overridden = NewObject<USkeletalBodySetup>(GetTransientPackage());
    Overridden->BoneName = TEXT("probe_overridden");
    Overridden->PhysMaterial = Shared;
    Overridden->DefaultInstance.SetPhysMaterialOverride(Override);
    Overridden->AggGeom.SphereElems.AddDefaulted_GetRef().Radius = 10.0f;
    USkeletalBodySetup* Fallback = NewObject<USkeletalBodySetup>(GetTransientPackage());
    Fallback->BoneName = TEXT("probe_fallback");
    Fallback->PhysMaterial = Shared;
    Fallback->AggGeom.SphereElems.AddDefaulted_GetRef().Radius = 10.0f;
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    Value->SetStringField(TEXT("shared_path"), ObjectPath(Shared));
    Value->SetStringField(TEXT("override_path"), ObjectPath(Override));
    Value->SetObjectField(TEXT("overridden_body"), CapturePhysicsBody(Overridden, 0));
    Value->SetObjectField(TEXT("fallback_body"), CapturePhysicsBody(Fallback, 1));
    Override->SleepLinearVelocityThreshold = 0.0f;
    Override->SleepAngularVelocityThreshold = 0.0f;
    Value->SetObjectField(TEXT("zero_material_body"), CapturePhysicsBody(Overridden, 2));
    Fallback->DefaultInstance.SleepFamily = ESleepFamily::Custom;
    Fallback->DefaultInstance.CustomSleepThresholdMultiplier = 0.0f;
    Value->SetObjectField(TEXT("zero_multiplier_body"), CapturePhysicsBody(Fallback, 3));
    return Value;
}

TSharedPtr<FJsonObject> CapturePhysicsBodyProbe(UObject* Asset, const FString& RequestPath)
{
    UPhysicsAsset* PhysicsAsset = Cast<UPhysicsAsset>(Asset);
    FString Text;
    TSharedPtr<FJsonObject> Request;
    if (PhysicsAsset == nullptr || !FFileHelper::LoadFileToString(Text, *RequestPath)
        || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) || !Request.IsValid())
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Physics body probe requires a physics asset and a JSON request."));
        return nullptr;
    }
    const TArray<TSharedPtr<FJsonValue>>* Scales = nullptr;
    if (!Request->TryGetArrayField(TEXT("scales"), Scales)) { return nullptr; }
    bool RescaleFromUnit = false;
    Request->TryGetBoolField(TEXT("rescale_from_unit"), RescaleFromUnit);
    TArray<TSharedPtr<FJsonValue>> Samples;
    for (const TSharedPtr<FJsonValue>& ScaleValue : *Scales)
    {
        const TArray<TSharedPtr<FJsonValue>>* Axes = nullptr;
        if (!ScaleValue->TryGetArray(Axes) || Axes->Num() != 3) { return nullptr; }
        const FVector Scale((*Axes)[0]->AsNumber(), (*Axes)[1]->AsNumber(), (*Axes)[2]->AsNumber());
        if (Scale.ContainsNaN() || Scale.GetMin() <= 0.0) { return nullptr; }
        TSharedRef<FJsonObject> Sample = MakeShared<FJsonObject>();
        Sample->SetObjectField(TEXT("scale"), CaptureVector(Scale));
        TArray<TSharedPtr<FJsonValue>> Bodies;
        for (USkeletalBodySetup* Setup : PhysicsAsset->SkeletalBodySetups)
        {
            FBodyInstance Body;
            Body.CopyBodyInstancePropertiesFrom(&Setup->DefaultInstance);
            Body.bSimulatePhysics = false;
            Body.InitBody(Setup, FTransform(FQuat::Identity, FVector::ZeroVector,
                RescaleFromUnit ? FVector::OneVector : Scale), nullptr, nullptr, FInitBodySpawnParams(false, true));
            if (!Body.IsValidBodyInstance())
            {
                UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Physics body probe failed to initialize %s."), *Setup->BoneName.ToString());
                return nullptr;
            }
            if (RescaleFromUnit) { Body.UpdateBodyScale(Scale, true); }
            Body.UpdateMassProperties();
            TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
            Value->SetStringField(TEXT("bone_name"), Setup->BoneName.ToString());
            Value->SetNumberField(TEXT("mass_kg"), Body.GetBodyMass());
            Value->SetObjectField(TEXT("mass_space_local"), CaptureTransform(Body.GetMassSpaceLocal()));
            Value->SetObjectField(TEXT("principal_inertia_kg_cm2"), CaptureVector(Body.GetBodyInertiaTensor()));
            Value->SetObjectField(TEXT("body_scale"), CaptureVector(Body.Scale3D));
            Body.TermBody();
            Bodies.Add(MakeShared<FJsonValueObject>(Value));
        }
        Sample->SetArrayField(TEXT("bodies"), Bodies);
        Samples.Add(MakeShared<FJsonValueObject>(Sample));
        UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Physics body probe captured %d bodies at %s."), Bodies.Num(), *Scale.ToString());
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetBoolField(TEXT("rescale_from_unit"), RescaleFromUnit);
    Result->SetArrayField(TEXT("samples"), Samples);
    return Result;
}

TSharedPtr<FJsonObject> CaptureMeshRayVisits(const Chaos::FTriangleMeshImplicitObject& Mesh,
    const Chaos::FVec3& Start, const Chaos::FVec3& Direction, Chaos::FReal Length)
{
    struct FVisitor
    {
        const Chaos::FTriangleMeshImplicitObject& Mesh;
        Chaos::FVec3 Start, Direction;
        TArray<TSharedPtr<FJsonValue>> Visits;
        Chaos::FReal Time = TNumericLimits<Chaos::FReal>::Max();
        Chaos::FVec3 Normal = Chaos::FVec3::ZeroVector;
        int32 Face = INDEX_NONE;

        bool VisitRaycast(int32 FaceIndex, float& CurrentLength)
        {
            Chaos::FTriangle Triangle;
            int32 A, B, C;
            Mesh.GetTransformedTriangle(FaceIndex, Chaos::FRigidTransform3::Identity, Triangle, A, B, C);
            const Chaos::FVec3 FaceNormal = Chaos::FVec3::CrossProduct(Triangle[1] - Triangle[0], Triangle[2] - Triangle[0]);
            const bool bCulled = Mesh.GetCullsBackFaceRaycast() && Chaos::FVec3::DotProduct(Direction, FaceNormal) > 0;
            Chaos::FReal HitTime;
            Chaos::FVec3 HitNormal;
            Chaos::TVec2<Chaos::FReal> Bary;
            const bool bHit = !bCulled && Chaos::RayTriangleIntersectionAndBary(Start, Direction,
                static_cast<Chaos::FReal>(CurrentLength), Triangle[0], Triangle[1], Triangle[2], HitTime, Bary, HitNormal);
            TSharedRef<FJsonObject> Visit = MakeShared<FJsonObject>();
            Visit->SetNumberField(TEXT("internal_face"), FaceIndex);
            Visit->SetNumberField(TEXT("external_face"), Mesh.GetExternalFaceIndexFromInternal(FaceIndex));
            Visit->SetNumberField(TEXT("length_before_cm"), CurrentLength);
            Visit->SetBoolField(TEXT("culled"), bCulled);
            Visit->SetBoolField(TEXT("hit"), bHit);
            TArray<TSharedPtr<FJsonValue>> Vertices;
            for (int32 Index = 0; Index < 3; ++Index) { Vertices.Add(MakeShared<FJsonValueObject>(CaptureVector(FVector(Triangle[Index])))); }
            Visit->SetArrayField(TEXT("vertices_cm"), Vertices);
            if (bHit)
            {
                Time = HitTime;
                Normal = HitNormal;
                Face = FaceIndex;
                CurrentLength = static_cast<float>(HitTime);
                Visit->SetNumberField(TEXT("distance_cm"), HitTime);
                Visit->SetObjectField(TEXT("normal"), CaptureVector(FVector(HitNormal)));
                TArray<TSharedPtr<FJsonValue>> BaryValues;
                BaryValues.Add(MakeShared<FJsonValueNumber>(Bary.X));
                BaryValues.Add(MakeShared<FJsonValueNumber>(Bary.Y));
                Visit->SetArrayField(TEXT("barycentric_uv"), BaryValues);
            }
            Visit->SetNumberField(TEXT("length_after_cm"), CurrentLength);
            Visits.Add(MakeShared<FJsonValueObject>(Visit));
            return true;
        }
    };
    FVisitor Visitor{Mesh, Start, Direction};
    Mesh.GetBVH().Raycast(Start, Direction, Length, Visitor);
    Chaos::FReal Time;
    Chaos::FVec3 Position, Normal;
    int32 Face;
    const bool bHit = Mesh.Raycast(Start, Direction, Length, 0, Time, Position, Normal, Face);
    if (bHit != (Visitor.Time <= Length)
        || (bHit && (Time != Visitor.Time || Face != Visitor.Face || Normal != Visitor.Normal)))
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Mesh ray visit capture differs from the native core query."));
        return nullptr;
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetObjectField(TEXT("start_cm"), CaptureVector(FVector(Start)));
    Result->SetObjectField(TEXT("direction"), CaptureVector(FVector(Direction)));
    Result->SetNumberField(TEXT("length_cm"), Length);
    Result->SetBoolField(TEXT("agrees_with_core"), true);
    Result->SetBoolField(TEXT("hit"), bHit);
    Result->SetNumberField(TEXT("selected_internal_face"), Visitor.Face);
    if (bHit)
    {
        Result->SetNumberField(TEXT("distance_cm"), Time);
        Result->SetObjectField(TEXT("position_cm"), CaptureVector(FVector(Position)));
        Result->SetObjectField(TEXT("normal"), CaptureVector(FVector(Normal)));
    }
    Result->SetArrayField(TEXT("visits"), Visitor.Visits);
    return Result;
}

TSharedPtr<FJsonObject> CaptureGeometryQueryProbe(const FString& RequestPath)
{
    FString Text;
    TSharedPtr<FJsonObject> Request;
    if (!FFileHelper::LoadFileToString(Text, *RequestPath)
        || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) || !Request.IsValid()) { return nullptr; }
    const TArray<TSharedPtr<FJsonValue>>* Inputs = nullptr;
    if (!Request->TryGetArrayField(TEXT("samples"), Inputs) || Inputs->IsEmpty()) { return nullptr; }
    const auto ReadArray = [](const TSharedPtr<FJsonObject>& Object, const TCHAR* Key, TArray<double>& Values, int32 Count)
    {
        const TArray<TSharedPtr<FJsonValue>>* Array = nullptr;
        if (!Object->TryGetArrayField(Key, Array) || Array->Num() != Count) { return false; }
        Values.SetNumUninitialized(Count);
        for (int32 Index = 0; Index < Count; ++Index)
        {
            if (!(*Array)[Index]->TryGetNumber(Values[Index]) || !FMath::IsFinite(Values[Index])) { return false; }
        }
        return true;
    };
    TArray<TSharedPtr<FJsonValue>> Samples;
    for (const TSharedPtr<FJsonValue>& InputValue : *Inputs)
    {
        const TSharedPtr<FJsonObject>* Input = nullptr;
        TArray<double> RotationValues;
        if (!InputValue->TryGetObject(Input) || !ReadArray(*Input, TEXT("rotation_xyzw"), RotationValues, 4)) { return nullptr; }
        const FQuat Rotation(RotationValues[0], RotationValues[1], RotationValues[2], RotationValues[3]);
        if (!Rotation.IsNormalized())
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Geometry query probe has an invalid rotation."));
            return nullptr;
        }
        TSharedRef<FJsonObject> Sample = MakeShared<FJsonObject>();
        Sample->SetObjectField(TEXT("input"), Input->ToSharedRef());
        Sample->SetObjectField(TEXT("rotator_degrees"), CaptureRotator(Rotation.Rotator()));
        if ((*Input)->HasField(TEXT("euler_degrees")))
        {
            TArray<double> Euler;
            if (!ReadArray(*Input, TEXT("euler_degrees"), Euler, 3)) { return nullptr; }
            Sample->SetObjectField(TEXT("rotation_from_euler"), CaptureQuaternion(FRotator(Euler[0], Euler[1], Euler[2]).Quaternion()));
        }
        if ((*Input)->HasField(TEXT("scaled_distance")))
        {
            const TSharedPtr<FJsonObject>* Distance = nullptr;
            TArray<double> A, B, Scale;
            if (!(*Input)->TryGetObjectField(TEXT("scaled_distance"), Distance)
                || !ReadArray(*Distance, TEXT("a"), A, 3) || !ReadArray(*Distance, TEXT("b"), B, 3)
                || !ReadArray(*Distance, TEXT("scale"), Scale, 3)) { return nullptr; }
            const double Value = ((FVector(A[0], A[1], A[2]) - FVector(B[0], B[1], B[2])) * FVector(Scale[0], Scale[1], Scale[2])).Size();
            if (!FMath::IsFinite(Value) || !FMath::IsFinite(static_cast<float>(Value))) { return nullptr; }
            Sample->SetNumberField(TEXT("scaled_distance"), Value);
            Sample->SetNumberField(TEXT("scaled_distance_float"), static_cast<float>(Value));
        }
        if ((*Input)->HasField(TEXT("float_range_map")))
        {
            const TSharedPtr<FJsonObject>* Mapping = nullptr;
            TArray<double> InRange, OutRange;
            double InValue;
            if (!(*Input)->TryGetObjectField(TEXT("float_range_map"), Mapping) ||
                !(*Mapping)->HasTypedField<EJson::Number>(TEXT("input")) || !(*Mapping)->TryGetNumberField(TEXT("input"), InValue) ||
                !ReadArray(*Mapping, TEXT("input_range"), InRange, 2) || !ReadArray(*Mapping, TEXT("output_range"), OutRange, 2) ||
                !FMath::IsFinite(static_cast<float>(InValue)) || !FMath::IsFinite(static_cast<float>(InRange[0])) ||
                !FMath::IsFinite(static_cast<float>(InRange[1])) || !FMath::IsFinite(static_cast<float>(OutRange[0])) ||
                !FMath::IsFinite(static_cast<float>(OutRange[1]))) { return nullptr; }
            const float Mapped = FMath::GetMappedRangeValueUnclamped(FVector2f(InRange[0], InRange[1]), FVector2f(OutRange[0], OutRange[1]), static_cast<float>(InValue));
            if (!FMath::IsFinite(Mapped)) { return nullptr; }
            Sample->SetNumberField(TEXT("float_range_mapped"), Mapped);
        }
        if ((*Input)->HasField(TEXT("additive_accumulation")))
        {
            const TSharedPtr<FJsonObject>* Accumulation = nullptr;
            const TArray<TSharedPtr<FJsonValue>>* Layers = nullptr;
            TArray<double> Translation, Scale;
            if (!(*Input)->TryGetObjectField(TEXT("additive_accumulation"), Accumulation)
                || !ReadArray(*Accumulation, TEXT("translation_cm"), Translation, 3)
                || !ReadArray(*Accumulation, TEXT("scale"), Scale, 3)
                || !(*Accumulation)->TryGetArrayField(TEXT("layers"), Layers)) { return nullptr; }
            FMemMark Mark(FMemStack::Get());
            FCompactPose BasePose, AdditivePose;
            FBlendedCurve BaseCurve, AdditiveCurve;
            BasePose.InitBones(1);
            AdditivePose.InitBones(1);
            const FCompactPoseBoneIndex Bone(0);
            BasePose[Bone] = FTransform(Rotation, FVector(Translation[0], Translation[1], Translation[2]), FVector(Scale[0], Scale[1], Scale[2]));
            for (const TSharedPtr<FJsonValue>& LayerValue : *Layers)
            {
                const TSharedPtr<FJsonObject>* Layer = nullptr;
                TArray<double> Quaternion, Offset, ScaleDelta;
                double Weight;
                if (!LayerValue->TryGetObject(Layer) || !ReadArray(*Layer, TEXT("rotation_xyzw"), Quaternion, 4)
                    || !ReadArray(*Layer, TEXT("translation_cm"), Offset, 3) || !ReadArray(*Layer, TEXT("scale_delta"), ScaleDelta, 3)
                    || !(*Layer)->TryGetNumberField(TEXT("weight"), Weight) || !FMath::IsFinite(static_cast<float>(Weight))) { return nullptr; }
                const FQuat AdditiveRotation(Quaternion[0], Quaternion[1], Quaternion[2], Quaternion[3]);
                if (!AdditiveRotation.IsNormalized()) { return nullptr; }
                AdditivePose[Bone] = FTransform(AdditiveRotation, FVector(Offset[0], Offset[1], Offset[2]), FVector(ScaleDelta[0], ScaleDelta[1], ScaleDelta[2]));
                FAnimationRuntime::AccumulateAdditivePose(BasePose, AdditivePose, BaseCurve, AdditiveCurve, static_cast<float>(Weight), AAT_LocalSpaceBase);
            }
            Sample->SetObjectField(TEXT("additive_accumulation"), CaptureTransform(BasePose[Bone]));
        }
        if ((*Input)->HasField(TEXT("yaw_difference")))
        {
            const TSharedPtr<FJsonObject>* Difference = nullptr;
            double VelocityYawDegrees, ViewYawRadians;
            if (!(*Input)->TryGetObjectField(TEXT("yaw_difference"), Difference)
                || !(*Difference)->HasTypedField<EJson::Number>(TEXT("velocity_yaw_degrees"))
                || !(*Difference)->HasTypedField<EJson::Number>(TEXT("view_yaw_radians"))
                || !(*Difference)->TryGetNumberField(TEXT("velocity_yaw_degrees"), VelocityYawDegrees)
                || !(*Difference)->TryGetNumberField(TEXT("view_yaw_radians"), ViewYawRadians)
                || !FMath::IsFinite(static_cast<float>(VelocityYawDegrees)) || !FMath::IsFinite(ViewYawRadians)) { return nullptr; }
            const double ViewYawDegrees = FMath::RadiansToDegrees(ViewYawRadians);
            const float DifferenceAngle = static_cast<float>(static_cast<float>(VelocityYawDegrees) - ViewYawDegrees);
            if (!FMath::IsFinite(ViewYawDegrees) || !FMath::IsFinite(DifferenceAngle)
                || (DifferenceAngle > 180.0f && DifferenceAngle - 360.0f == DifferenceAngle)
                || (DifferenceAngle < -180.0f && DifferenceAngle + 360.0f == DifferenceAngle)) { return nullptr; }
            const float Angle = FMath::UnwindDegrees(DifferenceAngle);
            Sample->SetNumberField(TEXT("view_yaw_degrees"), ViewYawDegrees);
            Sample->SetNumberField(TEXT("yaw_difference"), Angle);
        }
        if ((*Input)->HasField(TEXT("rotation_factors")))
        {
            const TArray<TSharedPtr<FJsonValue>>* Factors = nullptr;
            if (!(*Input)->TryGetArrayField(TEXT("rotation_factors"), Factors)) { return nullptr; }
            FQuat Product = Rotation;
            for (const TSharedPtr<FJsonValue>& FactorValue : *Factors)
            {
                const TSharedPtr<FJsonObject>* Factor = nullptr;
                TArray<double> Values;
                if (!FactorValue->TryGetObject(Factor) || !ReadArray(*Factor, TEXT("rotation_xyzw"), Values, 4)) { return nullptr; }
                const FQuat FactorRotation(Values[0], Values[1], Values[2], Values[3]);
                if (!FactorRotation.IsNormalized()) { return nullptr; }
                Product = Product * FactorRotation;
            }
            Sample->SetObjectField(TEXT("rotation_product"), CaptureQuaternion(Product));
            Sample->SetObjectField(TEXT("rotation_product_degrees"), CaptureRotator(Product.Rotator()));
            Sample->SetObjectField(TEXT("normalized_rotation_product"), CaptureQuaternion(Product.GetNormalized()));
        }
        if ((*Input)->HasField(TEXT("rotation_between_vectors")))
        {
            const TSharedPtr<FJsonObject>* Vectors = nullptr;
            TArray<double> From, To;
            if (!(*Input)->TryGetObjectField(TEXT("rotation_between_vectors"), Vectors)
                || !ReadArray(*Vectors, TEXT("from"), From, 3) || !ReadArray(*Vectors, TEXT("to"), To, 3)) { return nullptr; }
            Sample->SetObjectField(TEXT("rotation_between_vectors"), CaptureQuaternion(FQuat::FindBetweenVectors(
                FVector(From[0], From[1], From[2]), FVector(To[0], To[1], To[2]))));
        }
        if ((*Input)->HasField(TEXT("two_bone_ik")))
        {
            const TSharedPtr<FJsonObject>* Ik = nullptr;
            TArray<double> Root, End, Pole, Lengths, Stretch;
            bool bEnableStretch = false;
            if (!(*Input)->TryGetObjectField(TEXT("two_bone_ik"), Ik)
                || !ReadArray(*Ik, TEXT("root_cm"), Root, 3)
                || !ReadArray(*Ik, TEXT("effector_cm"), End, 3) || !ReadArray(*Ik, TEXT("pole_cm"), Pole, 3)
                || !ReadArray(*Ik, TEXT("lengths_cm"), Lengths, 2) || !ReadArray(*Ik, TEXT("stretch_ratios"), Stretch, 2)
                || !(*Ik)->TryGetBoolField(TEXT("stretch"), bEnableStretch)) { return nullptr; }
            const float LengthA = Lengths[0];
            const float LengthB = Lengths[1];
            const float StretchStart = Stretch[0];
            const float StretchMaximum = Stretch[1];
            if (!FMath::IsFinite(LengthA) || !FMath::IsFinite(LengthB) || !FMath::IsFinite(StretchStart)
                || !FMath::IsFinite(StretchMaximum) || LengthA < 0 || LengthB < 0) { return nullptr; }
            FTransform BoneA(FVector(Root[0], Root[1], Root[2]));
            FTransform BoneB;
            FTransform Effector(FVector(End[0], End[1], End[2]));
            BoneA.SetRotation(Rotation);
            BoneB.SetRotation(Rotation);
            FVector PrimaryAxis = FVector::ForwardVector;
            FVector SecondaryAxis = FVector::RightVector;
            float SecondaryWeight = 0;
            if ((*Ik)->HasField(TEXT("orientation")))
            {
                const TSharedPtr<FJsonObject>* Orientation = nullptr;
                TArray<double> Primary, Secondary;
                double Weight = 0;
                if (!(*Ik)->TryGetObjectField(TEXT("orientation"), Orientation)
                    || !ReadArray(*Orientation, TEXT("primary_axis"), Primary, 3)
                    || !ReadArray(*Orientation, TEXT("secondary_axis"), Secondary, 3)
                    || !(*Orientation)->TryGetNumberField(TEXT("secondary_weight"), Weight)) { return nullptr; }
                SecondaryWeight = Weight;
                if (!FMath::IsFinite(SecondaryWeight)) { return nullptr; }
                PrimaryAxis = FVector(Primary[0], Primary[1], Primary[2]);
                SecondaryAxis = FVector(Secondary[0], Secondary[1], Secondary[2]);
            }
            FControlRigMathLibrary::SolveBasicTwoBoneIK(BoneA, BoneB, Effector, FVector(Pole[0], Pole[1], Pole[2]),
                PrimaryAxis, SecondaryAxis, SecondaryWeight, LengthA, LengthB, bEnableStretch, StretchStart, StretchMaximum);
            TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
            Result->SetObjectField(TEXT("joint_cm"), CaptureVector(BoneB.GetLocation()));
            Result->SetObjectField(TEXT("effector_cm"), CaptureVector(Effector.GetLocation()));
            Result->SetObjectField(TEXT("root_rotation"), CaptureQuaternion(BoneA.GetRotation()));
            Result->SetObjectField(TEXT("joint_rotation"), CaptureQuaternion(BoneB.GetRotation()));
            Result->SetNumberField(TEXT("length_a_cm"), LengthA);
            Result->SetNumberField(TEXT("length_b_cm"), LengthB);
            Sample->SetObjectField(TEXT("two_bone_ik"), Result);
        }
        if ((*Input)->HasField(TEXT("mesh_raycast")))
        {
            const TSharedPtr<FJsonObject>* Ray = nullptr;
            FString MeshPath;
            TArray<double> TranslationValues, ScaleValues, StartValues, EndValues;
            bool bTraceComplex = false;
            bool bCaptureVisits = false;
            if (!(*Input)->TryGetObjectField(TEXT("mesh_raycast"), Ray)
                || !(*Ray)->TryGetStringField(TEXT("asset_path"), MeshPath)
                || !ReadArray(*Ray, TEXT("translation_cm"), TranslationValues, 3)
                || !ReadArray(*Ray, TEXT("scale"), ScaleValues, 3)
                || !ReadArray(*Ray, TEXT("start_cm"), StartValues, 3)
                || !ReadArray(*Ray, TEXT("end_cm"), EndValues, 3)
                || !(*Ray)->TryGetBoolField(TEXT("trace_complex"), bTraceComplex)) { return nullptr; }
            if ((*Ray)->HasField(TEXT("capture_ray_visits")) && !(*Ray)->TryGetBoolField(TEXT("capture_ray_visits"), bCaptureVisits)) { return nullptr; }
            const FVector Scale(ScaleValues[0], ScaleValues[1], ScaleValues[2]);
            if (Scale.X == 0 || Scale.Y == 0 || Scale.Z == 0) { return nullptr; }
            UStaticMesh* Mesh = LoadObject<UStaticMesh>(nullptr, *MeshPath);
            UBodySetup* Setup = Mesh ? Mesh->GetBodySetup() : nullptr;
            if (Setup == nullptr) { return nullptr; }
            FBodyInstance Body;
            Body.CopyBodyInstancePropertiesFrom(&Setup->DefaultInstance);
            Body.bSimulatePhysics = false;
            Body.InitBody(Setup, FTransform(Rotation, FVector(TranslationValues[0], TranslationValues[1], TranslationValues[2]), Scale),
                nullptr, nullptr, FInitBodySpawnParams(false, true));
            if (!Body.IsValidBodyInstance()) { return nullptr; }
            ON_SCOPE_EXIT { Body.TermBody(); };
            TArray<FPhysicsShapeHandle> Shapes;
            FPhysicsInterface::GetAllShapes_AssumedLocked(Body.GetPhysicsActorHandle(), Shapes);
            TArray<TSharedPtr<FJsonValue>> ShapeValues;
            for (const FPhysicsShapeHandle& Shape : Shapes)
            {
                TSharedRef<FJsonObject> ShapeValue = MakeShared<FJsonObject>();
                const Chaos::Filter::FShapeFilterData& Filter = Shape.Shape->GetShapeFilterData();
                ShapeValue->SetBoolField(TEXT("complex"), Filter.HasFlag(Chaos::EFilterFlags::ComplexCollision));
                ShapeValue->SetBoolField(TEXT("simple"), Filter.HasFlag(Chaos::EFilterFlags::SimpleCollision));
                ShapeValue->SetBoolField(TEXT("query_enabled"), FPhysicsInterface::IsQueryShape(Shape));
                const bool bTriangleMesh = FPhysicsInterface::GetGeometryCollection(Shape).GetType() == ECollisionShapeType::Trimesh;
                ShapeValue->SetBoolField(TEXT("triangle_mesh"), bTriangleMesh);
                if (bTriangleMesh)
                {
                    Chaos::FVec3 CoreScale;
                    Chaos::FReal Margin;
                    const Chaos::FTriangleMeshImplicitObject* Core = Chaos::UnwrapImplicit<Chaos::FTriangleMeshImplicitObject>(
                        *Shape.Shape->GetLeafGeometry(), CoreScale, Margin);
                    if (Core == nullptr) { return nullptr; }
                    ShapeValue->SetBoolField(TEXT("culls_back_face"), Core->GetCullsBackFaceRaycast());
                    ShapeValue->SetObjectField(TEXT("core_scale"), CaptureVector(FVector(CoreScale)));
                    if (bCaptureVisits)
                    {
                        const auto& Actor = Body.GetPhysicsActorHandle()->GetGameThreadAPI();
                        const FTransform LeafWorld = FTransform(Shape.Shape->GetLeafRelativeTransform()) * FTransform(Actor.R(), Actor.X());
                        const FVector Delta = FVector(EndValues[0], EndValues[1], EndValues[2]) - FVector(StartValues[0], StartValues[1], StartValues[2]);
                        const float Distance = static_cast<float>(Delta.Size());
                        if (Distance <= UE_KINDA_SMALL_NUMBER) { return nullptr; }
                        Chaos::FVec3 CoreStart = LeafWorld.InverseTransformPositionNoScale(FVector(StartValues[0], StartValues[1], StartValues[2]));
                        Chaos::FVec3 CoreDirection = LeafWorld.InverseTransformVectorNoScale(Delta) / Distance;
                        Chaos::FReal CoreLength = Distance;
                        Chaos::FReal LengthScale = 1;
                        if (Chaos::IsScaled(Shape.Shape->GetLeafGeometry()->GetType()))
                        {
                            const Chaos::FVec3f InverseScale = static_cast<const Chaos::FImplicitObjectScaled&>(*Shape.Shape->GetLeafGeometry()).GetInvScale();
                            ShapeValue->SetObjectField(TEXT("core_inverse_scale"), CaptureVector(FVector(InverseScale)));
                            CoreStart = InverseScale * CoreStart;
                            CoreDirection = InverseScale * CoreDirection;
                            LengthScale = CoreDirection.Size();
                            CoreDirection *= Chaos::FReal(1) / LengthScale;
                            CoreLength *= LengthScale;
                        }
                        const TSharedPtr<FJsonObject> Visits = CaptureMeshRayVisits(*Core, CoreStart, CoreDirection, CoreLength);
                        if (!Visits.IsValid()) { return nullptr; }
                        Visits->SetNumberField(TEXT("length_scale"), LengthScale);
                        ShapeValue->SetObjectField(TEXT("ray_visits"), Visits);
                    }
                }
                ShapeValues.Add(MakeShared<FJsonValueObject>(ShapeValue));
            }
            FHitResult Hit;
            const bool bHit = Body.LineTrace(Hit, FVector(StartValues[0], StartValues[1], StartValues[2]),
                FVector(EndValues[0], EndValues[1], EndValues[2]), bTraceComplex, false);
            TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
            Result->SetArrayField(TEXT("shapes"), ShapeValues);
            Result->SetBoolField(TEXT("double_sided_geometry"), Setup->bDoubleSidedGeometry);
            Result->SetNumberField(TEXT("collision_trace_flag"), static_cast<int32>(Setup->GetCollisionTraceFlag()));
            Result->SetBoolField(TEXT("hit"), bHit);
            if (bHit)
            {
                Result->SetNumberField(TEXT("distance_cm"), Hit.Distance);
                Result->SetObjectField(TEXT("position_cm"), CaptureVector(Hit.ImpactPoint));
                Result->SetObjectField(TEXT("normal"), CaptureVector(Hit.ImpactNormal));
                Result->SetNumberField(TEXT("face_index"), Hit.FaceIndex);
            }
            Sample->SetObjectField(TEXT("mesh_raycast"), Result);
            Samples.Add(MakeShared<FJsonValueObject>(Sample));
            continue;
        }
        const bool bRay = (*Input)->HasField(TEXT("box_raycast"));
        const bool bCapsule = (*Input)->HasField(TEXT("capsule_sweep"));
        const TCHAR* SweepKey = bRay ? TEXT("box_raycast") : bCapsule ? TEXT("capsule_sweep") : TEXT("sphere_sweep");
        if ((*Input)->HasField(SweepKey))
        {
            const TSharedPtr<FJsonObject>* Sweep = nullptr;
            TArray<double> BoxTranslationValues, BoxHalfExtentsValues, StartValues, DirectionValues;
            double Radius = 0;
            double HalfHeight = 0;
            double Length = 0;
            if (!(*Input)->TryGetObjectField(SweepKey, Sweep)
                || !ReadArray(*Sweep, TEXT("box_translation_cm"), BoxTranslationValues, 3)
                || !ReadArray(*Sweep, TEXT("box_half_extents_cm"), BoxHalfExtentsValues, 3)
                || !ReadArray(*Sweep, TEXT("start_cm"), StartValues, 3)
                || !ReadArray(*Sweep, TEXT("direction"), DirectionValues, 3)
                || (!bRay && (!(*Sweep)->TryGetNumberField(TEXT("radius_cm"), Radius) || !FMath::IsFinite(Radius) || Radius <= 0))
                || (bCapsule && (!(*Sweep)->TryGetNumberField(TEXT("half_height_cm"), HalfHeight) || !FMath::IsFinite(HalfHeight) || HalfHeight <= 0))
                || !(*Sweep)->TryGetNumberField(TEXT("length_cm"), Length) || !FMath::IsFinite(Length) || Length <= 0)
            {
                UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Geometry query probe has an invalid sweep."));
                return nullptr;
            }
            const FVector BoxTranslation(BoxTranslationValues[0], BoxTranslationValues[1], BoxTranslationValues[2]);
            const FVector BoxHalfExtents(BoxHalfExtentsValues[0], BoxHalfExtentsValues[1], BoxHalfExtentsValues[2]);
            const FVector Start(StartValues[0], StartValues[1], StartValues[2]);
            const FVector Direction(DirectionValues[0], DirectionValues[1], DirectionValues[2]);
            if (BoxHalfExtents.GetMin() <= 0 || !Direction.IsNormalized()) { return nullptr; }
            FPhysicsShapeAdapter BoxAdapter(FQuat::Identity, FCollisionShape::MakeBox(BoxHalfExtents));
            Chaos::FReal Time = Length;
            Chaos::FVec3 Position = Chaos::FVec3::ZeroVector;
            Chaos::FVec3 Normal = Chaos::FVec3::ZeroVector;
            Chaos::FVec3 FaceNormal = Chaos::FVec3::ZeroVector;
            int32 FaceIndex = INDEX_NONE;
            const FTransform BoxTransform(Rotation, BoxTranslation);
            bool Hit = false;
            if (bRay)
            {
                Hit = BoxAdapter.GetGeometry().Raycast(BoxTransform.InverseTransformPositionNoScale(Start),
                    BoxTransform.InverseTransformVectorNoScale(Direction), Length, 0.0, Time, Position, Normal, FaceIndex);
                if (Hit)
                {
                    Position = BoxTransform.TransformPositionNoScale(Position);
                    Normal = BoxTransform.TransformVectorNoScale(Normal);
                }
            }
            else
            {
                FPhysicsShapeAdapter QueryAdapter(FQuat::Identity, bCapsule
                    ? FCollisionShape::MakeCapsule(Radius, HalfHeight) : FCollisionShape::MakeSphere(Radius));
                Hit = Chaos::SweepQuery(BoxAdapter.GetGeometry(), BoxTransform,
                    QueryAdapter.GetGeometry(), QueryAdapter.GetGeomPose(Start), Direction, Length, Time, Position, Normal,
                    FaceIndex, FaceNormal, 0.0, true);
            }
            TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
            Result->SetBoolField(TEXT("hit"), Hit);
            if (Hit)
            {
                Result->SetNumberField(TEXT("distance_cm"), Time);
                Result->SetObjectField(TEXT("position_cm"), CaptureVector(FVector(Position)));
                Result->SetObjectField(TEXT("normal"), CaptureVector(FVector(Normal)));
            }
            Sample->SetObjectField(SweepKey, Result);
        }
        Samples.Add(MakeShared<FJsonValueObject>(Sample));
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetArrayField(TEXT("samples"), Samples);
    return Result;
}

TSharedPtr<FJsonObject> CapturePhysicsContactProbe(UObject* Asset, const FString& RequestPath)
{
    UPhysicsAsset* PhysicsAsset = Cast<UPhysicsAsset>(Asset);
    FString Text;
    TSharedPtr<FJsonObject> Request;
    const TArray<TSharedPtr<FJsonValue>>* Inputs = nullptr;
    if (PhysicsAsset == nullptr || !FFileHelper::LoadFileToString(Text, *RequestPath)
        || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) || !Request.IsValid()
        || !Request->TryGetArrayField(TEXT("samples"), Inputs) || Inputs->IsEmpty() || Inputs->Num() > 128)
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Physics contact probe requires a physics asset and 1 to 128 samples."));
        return nullptr;
    }
    const auto ReadVector = [](const TSharedPtr<FJsonObject>& Object, const TCHAR* Key, FVector& Value)
    {
        const TArray<TSharedPtr<FJsonValue>>* Axes = nullptr;
        if (!Object->TryGetArrayField(Key, Axes) || Axes->Num() != 3) { return false; }
        for (int32 Axis = 0; Axis < 3; ++Axis)
        {
            double Number = 0;
            if (!(*Axes)[Axis]->TryGetNumber(Number) || !FMath::IsFinite(Number)) { return false; }
            Value[Axis] = Number;
        }
        return true;
    };
    const auto ReadRotation = [](const TSharedPtr<FJsonObject>& Object, const TCHAR* Key, FQuat& Value)
    {
        const TArray<TSharedPtr<FJsonValue>>* Axes = nullptr;
        if (!Object->TryGetArrayField(Key, Axes) || Axes->Num() != 4) { return false; }
        double Values[4];
        for (int32 Axis = 0; Axis < 4; ++Axis)
        {
            if (!(*Axes)[Axis]->TryGetNumber(Values[Axis]) || !FMath::IsFinite(Values[Axis])) { return false; }
        }
        Value = FQuat(Values[0], Values[1], Values[2], Values[3]);
        return Value.IsNormalized();
    };
    TArray<TSharedPtr<FJsonValue>> Samples;
    for (const TSharedPtr<FJsonValue>& InputValue : *Inputs)
    {
        const TSharedPtr<FJsonObject>* InputPtr = nullptr;
        if (!InputValue->TryGetObject(InputPtr)) { return nullptr; }
        const TSharedPtr<FJsonObject>& Input = *InputPtr;
        FString BoneName;
        int32 ShapeIndex = 0;
        double CullDistance = 0;
        double DeltaSeconds = 0;
        const bool HasMesh = Input->HasField(TEXT("mesh_asset_path"));
        bool AggregateMesh = false;
        Input->TryGetBoolField(TEXT("aggregate_mesh"), AggregateMesh);
        if (AggregateMesh && !HasMesh) { return nullptr; }
        const TCHAR* OtherTranslationKey = HasMesh ? TEXT("mesh_translation_cm") : TEXT("box_translation_cm");
        const TCHAR* OtherRotationKey = HasMesh ? TEXT("mesh_rotation_xyzw") : TEXT("box_rotation_xyzw");
        FVector BodyTranslation, OtherTranslation, BoxHalfExtents(1.0), MeshScale(1.0);
        FQuat BodyRotation, OtherRotation;
        if (!Input->TryGetStringField(TEXT("bone_name"), BoneName)
            || !Input->TryGetNumberField(TEXT("shape_index"), ShapeIndex) || ShapeIndex < 0
            || !Input->TryGetNumberField(TEXT("cull_distance_cm"), CullDistance) || !FMath::IsFinite(CullDistance) || CullDistance < 0
            || !Input->TryGetNumberField(TEXT("delta_seconds"), DeltaSeconds) || !FMath::IsFinite(DeltaSeconds) || DeltaSeconds <= 0
            || !ReadVector(Input, TEXT("body_translation_cm"), BodyTranslation)
            || !ReadVector(Input, OtherTranslationKey, OtherTranslation)
            || (!HasMesh && (!ReadVector(Input, TEXT("box_half_extents_cm"), BoxHalfExtents) || BoxHalfExtents.GetMin() <= 0))
            || (HasMesh && (!ReadVector(Input, TEXT("mesh_scale"), MeshScale) || MeshScale.GetAbsMin() <= 0))
            || !ReadRotation(Input, TEXT("body_rotation_xyzw"), BodyRotation)
            || !ReadRotation(Input, OtherRotationKey, OtherRotation))
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Physics contact probe has an invalid sample."));
            return nullptr;
        }
        FVector NextBodyTranslation, NextOtherTranslation;
        FQuat NextBodyRotation, NextOtherRotation;
        const bool HasNext = Input->HasField(TEXT("next"));
        const TSharedPtr<FJsonObject>* Next = nullptr;
        if (HasNext && (!Input->TryGetObjectField(TEXT("next"), Next)
            || !ReadVector(*Next, TEXT("body_translation_cm"), NextBodyTranslation)
            || !ReadVector(*Next, OtherTranslationKey, NextOtherTranslation)
            || !ReadRotation(*Next, TEXT("body_rotation_xyzw"), NextBodyRotation)
            || !ReadRotation(*Next, OtherRotationKey, NextOtherRotation)))
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Physics contact probe has an invalid next transform."));
            return nullptr;
        }
        const bool HasHistory = Input->HasField(TEXT("history"));
        const TSharedPtr<FJsonObject>* History = nullptr;
        const TArray<TSharedPtr<FJsonValue>>* FrictionRatios = nullptr;
        FVector AnchorOffset0, AnchorOffset1;
        if (HasHistory && (!HasNext || !Input->TryGetObjectField(TEXT("history"), History)
            || !(*History)->TryGetArrayField(TEXT("friction_ratios"), FrictionRatios)
            || !ReadVector(*History, TEXT("anchor_offset_0_cm"), AnchorOffset0)
            || !ReadVector(*History, TEXT("anchor_offset_1_cm"), AnchorOffset1)))
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Physics contact probe has an invalid history input."));
            return nullptr;
        }
        USkeletalBodySetup* Setup = nullptr;
        for (USkeletalBodySetup* Candidate : PhysicsAsset->SkeletalBodySetups)
        {
            if (Candidate->BoneName.ToString() == BoneName) { Setup = Candidate; break; }
        }
        if (Setup == nullptr) { return nullptr; }
        FBodyInstance Body;
        Body.CopyBodyInstancePropertiesFrom(&Setup->DefaultInstance);
        Body.bSimulatePhysics = false;
        Body.InitBody(Setup, FTransform::Identity, nullptr, nullptr, FInitBodySpawnParams(false, true));
        if (!Body.IsValidBodyInstance()) { return nullptr; }
        TArray<FPhysicsShapeHandle> Shapes;
        FPhysicsInterface::GetAllShapes_AssumedLocked(Body.GetPhysicsActorHandle(), Shapes);
        if (!Shapes.IsValidIndex(ShapeIndex)) { Body.TermBody(); return nullptr; }
        const FPhysicsShapeHandle& Shape = Shapes[ShapeIndex];
        const Chaos::FCollisionData BodyCollisionData = Shape.Shape->GetCollisionData();
        const FPhysicsGeometryCollection Geometry = FPhysicsInterface::GetGeometryCollection(Shape);
        const FTransform ShapeLocal = FPhysicsInterface::GetLocalTransform(Shape);
        Chaos::FImplicitObjectPtr Implicit;
        TSharedRef<FJsonObject> GeometryValue = MakeShared<FJsonObject>();
        if (Geometry.GetType() == ECollisionShapeType::Capsule)
        {
            const Chaos::FCapsule& Capsule = Geometry.GetCapsuleGeometry();
            Implicit = const_cast<Chaos::FCapsule*>(&Capsule);
            GeometryValue->SetStringField(TEXT("kind"), TEXT("capsule"));
            GeometryValue->SetObjectField(TEXT("endpoint_0_cm"), CaptureVector(FVector(Capsule.GetX1f())));
            GeometryValue->SetObjectField(TEXT("endpoint_1_cm"), CaptureVector(FVector(Capsule.GetX2f())));
            GeometryValue->SetNumberField(TEXT("radius_cm"), Capsule.GetRadiusf());
        }
        else if (Geometry.GetType() == ECollisionShapeType::Sphere)
        {
            const Chaos::FSphere& Sphere = Geometry.GetSphereGeometry();
            Implicit = const_cast<Chaos::FSphere*>(&Sphere);
            GeometryValue->SetStringField(TEXT("kind"), TEXT("sphere"));
            GeometryValue->SetObjectField(TEXT("center_cm"), CaptureVector(FVector(Sphere.GetCenterf())));
            GeometryValue->SetNumberField(TEXT("radius_cm"), Sphere.GetRadiusf());
        }
        else if (Geometry.GetType() == ECollisionShapeType::Box)
        {
            const Chaos::FImplicitBox3& Box = Geometry.GetBoxGeometry();
            Implicit = const_cast<Chaos::FImplicitBox3*>(&Box);
            GeometryValue->SetStringField(TEXT("kind"), TEXT("box"));
            GeometryValue->SetObjectField(TEXT("minimum_cm"), CaptureVector(FVector(Box.Min())));
            GeometryValue->SetObjectField(TEXT("maximum_cm"), CaptureVector(FVector(Box.Max())));
        }
        else { Body.TermBody(); return nullptr; }
        GeometryValue->SetObjectField(TEXT("shape_to_body"), CaptureTransform(ShapeLocal));
        Body.TermBody();
        FPhysicsShapeAdapter BoxAdapter(FQuat::Identity, FCollisionShape::MakeBox(BoxHalfExtents));
        Chaos::FImplicitObjectPtr Other = const_cast<Chaos::FImplicitObject*>(&BoxAdapter.GetGeometry());
        Chaos::FImplicitObjectPtr MeshRoot;
        TArray<Chaos::FCollisionData> MeshCollisionData;
        FTransform OtherLocal = FTransform::Identity;
        TSharedPtr<FJsonObject> OtherGeometryValue;
        if (HasMesh)
        {
            FString MeshPath;
            int32 MeshShapeIndex = 0;
            if (!Input->TryGetStringField(TEXT("mesh_asset_path"), MeshPath)
                || !Input->TryGetNumberField(TEXT("mesh_shape_index"), MeshShapeIndex) || MeshShapeIndex < 0) { return nullptr; }
            UStaticMesh* Mesh = LoadObject<UStaticMesh>(nullptr, *MeshPath);
            UBodySetup* MeshSetup = Mesh ? Mesh->GetBodySetup() : nullptr;
            if (MeshSetup == nullptr) { return nullptr; }
            FBodyInstance MeshBody;
            MeshBody.CopyBodyInstancePropertiesFrom(&MeshSetup->DefaultInstance);
            MeshBody.bSimulatePhysics = false;
            MeshBody.InitBody(MeshSetup, FTransform(FQuat::Identity, FVector::ZeroVector, MeshScale), nullptr, nullptr, FInitBodySpawnParams(false, true));
            if (!MeshBody.IsValidBodyInstance()) { return nullptr; }
            ON_SCOPE_EXIT { MeshBody.TermBody(); };
            TArray<FPhysicsShapeHandle> MeshShapes;
            FPhysicsInterface::GetAllShapes_AssumedLocked(MeshBody.GetPhysicsActorHandle(), MeshShapes);
            if (AggregateMesh)
            {
                MeshRoot = const_cast<Chaos::FImplicitObject*>(MeshBody.GetPhysicsActorHandle()->GetGameThreadAPI().GetGeometry());
                for (const FPhysicsShapeHandle& Entry : MeshShapes) { MeshCollisionData.Add(Entry.Shape->GetCollisionData()); }
            }
            if (!MeshShapes.IsValidIndex(MeshShapeIndex)) { return nullptr; }
            const FPhysicsShapeHandle& MeshShape = MeshShapes[MeshShapeIndex];
            const FPhysicsGeometryCollection MeshGeometry = FPhysicsInterface::GetGeometryCollection(MeshShape);
            Other = const_cast<Chaos::FImplicitObject*>(MeshShape.Shape->GetLeafGeometry());
            OtherLocal = MeshShape.Shape->GetLeafRelativeTransform();
            const bool IsConvex = MeshGeometry.GetType() == ECollisionShapeType::Convex;
            const bool IsTriangleMesh = MeshGeometry.GetType() == ECollisionShapeType::Trimesh;
            if ((!IsConvex && !IsTriangleMesh) || !Chaos::DoCollide(IsConvex ? Chaos::ImplicitObjectType::Convex : Chaos::ImplicitObjectType::TriangleMesh, MeshShape.Shape)) { return nullptr; }
            OtherGeometryValue = MakeShared<FJsonObject>();
            OtherGeometryValue->SetStringField(TEXT("asset_path"), Mesh->GetPathName());
            OtherGeometryValue->SetStringField(TEXT("kind"), IsConvex ? TEXT("convex") : TEXT("trimesh"));
            OtherGeometryValue->SetNumberField(TEXT("shape_count"), MeshShapes.Num());
            OtherGeometryValue->SetNumberField(TEXT("shape_index"), MeshShapeIndex);
            OtherGeometryValue->SetNumberField(TEXT("margin_cm"), Other->GetMarginf());
            OtherGeometryValue->SetNumberField(TEXT("collision_trace_type"), static_cast<int32>(MeshShape.Shape->GetCollisionTraceType()));
            OtherGeometryValue->SetBoolField(TEXT("simulation_enabled"), FPhysicsInterface::IsSimulationShape(MeshShape));
            OtherGeometryValue->SetBoolField(TEXT("query_enabled"), FPhysicsInterface::IsQueryShape(MeshShape));
            OtherGeometryValue->SetObjectField(TEXT("bounds_min_cm"), CaptureVector(FVector(Other->BoundingBox().Min())));
            OtherGeometryValue->SetObjectField(TEXT("bounds_max_cm"), CaptureVector(FVector(Other->BoundingBox().Max())));
            OtherGeometryValue->SetObjectField(TEXT("shape_to_body"), CaptureTransform(OtherLocal));
            if (IsConvex)
            {
                Chaos::FVec3 CoreScale;
                Chaos::FReal WrapperMargin;
                const Chaos::FConvex* Core = Chaos::UnwrapImplicit<Chaos::FConvex>(*Other, CoreScale, WrapperMargin);
                if (Core == nullptr) { return nullptr; }
                const Chaos::FConvex& Convex = *Core;
                OtherGeometryValue->SetObjectField(TEXT("convex_core_scale"), CaptureVector(FVector(CoreScale)));
                TArray<TSharedPtr<FJsonValue>> Vertices;
                for (int32 VertexIndex = 0; VertexIndex < Convex.NumVertices(); ++VertexIndex)
                {
                    Vertices.Add(MakeShared<FJsonValueObject>(CaptureVector(FVector(Convex.GetVertex(VertexIndex)))));
                }
                TArray<TSharedPtr<FJsonValue>> Planes;
                for (int32 PlaneIndex = 0; PlaneIndex < Convex.NumPlanes(); ++PlaneIndex)
                {
                    TSharedPtr<FJsonObject> Plane = MakeShared<FJsonObject>();
                    const auto SourcePlane = Convex.GetPlane(PlaneIndex);
                    Plane->SetObjectField(TEXT("normal"), CaptureVector(FVector(SourcePlane.Normal())));
                    Plane->SetObjectField(TEXT("origin_cm"), CaptureVector(FVector(SourcePlane.X())));
                    TArray<TSharedPtr<FJsonValue>> Indices;
                    for (int32 Index = 0; Index < Convex.NumPlaneVertices(PlaneIndex); ++Index)
                    {
                        Indices.Add(MakeShared<FJsonValueNumber>(Convex.GetPlaneVertex(PlaneIndex, Index)));
                    }
                    Plane->SetArrayField(TEXT("vertex_indices"), Indices);
                    Planes.Add(MakeShared<FJsonValueObject>(Plane));
                }
                OtherGeometryValue->SetArrayField(TEXT("convex_core_vertices_cm"), Vertices);
                OtherGeometryValue->SetArrayField(TEXT("convex_core_planes"), Planes);
            }
            else
            {
                Chaos::FVec3 CoreScale;
                Chaos::FReal WrapperMargin;
                const Chaos::FTriangleMeshImplicitObject* Core = Chaos::UnwrapImplicit<Chaos::FTriangleMeshImplicitObject>(*Other, CoreScale, WrapperMargin);
                if (Core == nullptr) { return nullptr; }
                OtherGeometryValue->SetObjectField(TEXT("trimesh_core_scale"), CaptureVector(FVector(CoreScale)));
                TArray<TSharedPtr<FJsonValue>> Vertices;
                for (uint32 Index = 0; Index < Core->Particles().Size(); ++Index)
                {
                    Vertices.Add(MakeShared<FJsonValueObject>(CaptureVector(FVector(Core->Particles().GetX(Index)))));
                }
                TArray<TSharedPtr<FJsonValue>> Triangles;
                for (int32 Index = 0; Index < Core->Elements().GetNumTriangles(); ++Index)
                {
                    Chaos::FTriangle Triangle;
                    int32 Vertex0, Vertex1, Vertex2;
                    Core->GetTransformedTriangle(Index, Chaos::FRigidTransform3::Identity, Triangle, Vertex0, Vertex1, Vertex2);
                    TArray<TSharedPtr<FJsonValue>> Indices;
                    for (int32 Vertex : {Vertex0, Vertex1, Vertex2}) { Indices.Add(MakeShared<FJsonValueNumber>(Vertex)); }
                    Triangles.Add(MakeShared<FJsonValueArray>(Indices));
                }
                OtherGeometryValue->SetArrayField(TEXT("trimesh_core_vertices_cm"), Vertices);
                OtherGeometryValue->SetArrayField(TEXT("trimesh_core_triangles"), Triangles);
            }
        }
        FChaosSolversModule* Module = FChaosSolversModule::GetModule();
        Chaos::FPBDRigidsSolver* Solver = Module->CreateSolver(nullptr, -1.0, Chaos::EThreadingMode::SingleThread);
        ON_SCOPE_EXIT { Module->DestroySolver(Solver); };
        Chaos::FPBDRigidsSOAs& Particles = Solver->GetParticles();
        Chaos::FPBDRigidParticleHandle* Particle0 = Particles.CreateDynamicParticles(1)[0];
        Chaos::FGeometryParticleHandle* Particle1 = Particles.CreateStaticParticles(1)[0];
        Particle0->SetGeometry(Implicit);
        Particle1->SetGeometry(Other);
        bool Swap = false;
        const Chaos::EContactShapesType PairType = Chaos::Collisions::CalculateShapePairType(Particle0, Implicit, Particle1, Other, Swap);
        if (Swap || PairType == Chaos::EContactShapesType::Unknown) { return nullptr; }
        Chaos::Private::FCollisionContextAllocator Allocator(&Solver->GetEvolution()->GetCollisionConstraints(), 1, 0);
        auto Constraint = Allocator.CreateConstraint(Particle0, Implicit, Particle0->ShapeInstances()[0].Get(), nullptr,
            Chaos::FRigidTransform3::Identity, Particle1, Other, Particle1->ShapeInstances()[0].Get(), nullptr,
            Chaos::FRigidTransform3::Identity, CullDistance, true, PairType);
        if (!Constraint.IsValid()) { return nullptr; }
        const FTransform ShapeWorld = ShapeLocal * FTransform(BodyRotation, BodyTranslation);
        const FTransform BoxWorld = OtherLocal * FTransform(OtherRotation, OtherTranslation);
        Constraint->SetShapeWorldTransforms(ShapeWorld, BoxWorld);
        Chaos::Collisions::UpdateConstraint(*Constraint, ShapeWorld, BoxWorld, DeltaSeconds);
        const auto CapturePoint = [](const auto& Contact, const FTransform& ShapeTransform, const FTransform& BoxTransform)
        {
            TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
            Value->SetNumberField(TEXT("phi_cm"), Contact.Phi);
            Value->SetNumberField(TEXT("contact_type"), static_cast<int32>(Contact.ContactType));
            Value->SetNumberField(TEXT("face_index"), Contact.FaceIndex);
            Value->SetObjectField(TEXT("shape_point_0_cm"), CaptureVector(FVector(Contact.ShapeContactPoints[0])));
            Value->SetObjectField(TEXT("shape_point_1_cm"), CaptureVector(FVector(Contact.ShapeContactPoints[1])));
            Value->SetObjectField(TEXT("world_point_0_cm"), CaptureVector(ShapeTransform.TransformPositionNoScale(FVector(Contact.ShapeContactPoints[0]))));
            Value->SetObjectField(TEXT("world_point_1_cm"), CaptureVector(BoxTransform.TransformPositionNoScale(FVector(Contact.ShapeContactPoints[1]))));
            Value->SetObjectField(TEXT("world_normal"), CaptureVector(BoxTransform.TransformVectorNoScale(FVector(Contact.ShapeContactNormal))));
            return MakeShared<FJsonValueObject>(Value);
        };
        const auto CapturePoints = [&CapturePoint](const Chaos::FPBDCollisionConstraint& ContactConstraint, const FTransform& ShapeTransform, const FTransform& BoxTransform)
        {
            TArray<TSharedPtr<FJsonValue>> Points;
            for (const Chaos::FManifoldPoint& Point : ContactConstraint.GetManifoldPoints())
            {
                if (Point.Flags.bDisabled) { continue; }
                Points.Add(CapturePoint(Point.ContactPoint, ShapeTransform, BoxTransform));
            }
            return Points;
        };
        const TArray<TSharedPtr<FJsonValue>> Points = CapturePoints(*Constraint, ShapeWorld, BoxWorld);
        TSharedRef<FJsonObject> Sample = MakeShared<FJsonObject>();
        Sample->SetObjectField(TEXT("input"), Input.ToSharedRef());
        Sample->SetObjectField(TEXT("geometry"), GeometryValue);
        if (OtherGeometryValue.IsValid()) { Sample->SetObjectField(TEXT("other_geometry"), OtherGeometryValue.ToSharedRef()); }
        Sample->SetArrayField(TEXT("points"), Points);
        Sample->SetNumberField(TEXT("collision_margin_0_cm"), Constraint->GetCollisionMargin0());
        Sample->SetNumberField(TEXT("collision_margin_1_cm"), Constraint->GetCollisionMargin1());
        const auto CaptureSimplex = [](Chaos::FPBDCollisionConstraint& ContactConstraint, const FTransform& ShapeTransform, const FTransform& BoxTransform)
        {
            TArray<TSharedPtr<FJsonValue>> Vertices;
            const Chaos::FGJKSimplexData& Simplex = ContactConstraint.GetGJKWarmStartData();
            for (int32 Index = 0; Index < Simplex.NumVerts; ++Index)
            {
                TSharedRef<FJsonObject> Vertex = MakeShared<FJsonObject>();
                Vertex->SetNumberField(TEXT("weight"), Simplex.Barycentric[Index]);
                Vertex->SetObjectField(TEXT("shape_point_0_cm"), CaptureVector(FVector(Simplex.As[Index])));
                Vertex->SetObjectField(TEXT("shape_point_1_cm"), CaptureVector(FVector(Simplex.Bs[Index])));
                Vertex->SetObjectField(TEXT("world_delta_cm"), CaptureVector(ShapeTransform.TransformPositionNoScale(FVector(Simplex.As[Index])) - BoxTransform.TransformPositionNoScale(FVector(Simplex.Bs[Index]))));
                Vertices.Add(MakeShared<FJsonValueObject>(Vertex));
            }
            return Vertices;
        };
        Sample->SetArrayField(TEXT("gjk_simplex"), CaptureSimplex(*Constraint, ShapeWorld, BoxWorld));
        if (Input->HasField(TEXT("triangle_indices")))
        {
            const TArray<TSharedPtr<FJsonValue>>* Indices = nullptr;
            const Chaos::FImplicitCapsule3* Capsule = Implicit->GetObject<Chaos::FImplicitCapsule3>();
            Chaos::FVec3 CoreScale;
            Chaos::FReal WrapperMargin;
            const Chaos::FTriangleMeshImplicitObject* Core = Chaos::UnwrapImplicit<Chaos::FTriangleMeshImplicitObject>(*Other, CoreScale, WrapperMargin);
            if (Capsule == nullptr || Core == nullptr || !Input->TryGetArrayField(TEXT("triangle_indices"), Indices)) { return nullptr; }
            Chaos::FRigidTransform3 MeshToCapsule = Chaos::FRigidTransform3(BoxWorld).GetRelativeTransformNoScale(ShapeWorld);
            MeshToCapsule.SetScale3D(CoreScale);
            TArray<TSharedPtr<FJsonValue>> TriangleContacts;
            for (const TSharedPtr<FJsonValue>& IndexValue : *Indices)
            {
                int32 Index = -1;
                if (!IndexValue->TryGetNumber(Index) || Index < 0 || Index >= Core->Elements().GetNumTriangles()) { return nullptr; }
                Chaos::FTriangle Triangle;
                int32 Vertex0, Vertex1, Vertex2;
                Core->GetTransformedTriangle(Index, MeshToCapsule, Triangle, Vertex0, Vertex1, Vertex2);
                Chaos::FContactPointManifold Contacts;
                Chaos::ConstructCapsuleTriangleOneShotManifold2(*Capsule, Triangle, CullDistance, Contacts);
                TArray<TSharedPtr<FJsonValue>> TrianglePoints;
                for (const Chaos::FContactPoint& Contact : Contacts) { TrianglePoints.Add(CapturePoint(Contact, ShapeWorld, ShapeWorld)); }
                TSharedRef<FJsonObject> TriangleValue = MakeShared<FJsonObject>();
                TriangleValue->SetNumberField(TEXT("triangle_index"), Index);
                TriangleValue->SetArrayField(TEXT("points"), TrianglePoints);
                TriangleContacts.Add(MakeShared<FJsonValueObject>(TriangleValue));
            }
            Sample->SetArrayField(TEXT("triangle_contacts"), TriangleContacts);
        }
        if (HasNext)
        {
            if (HasHistory)
            {
                if (FrictionRatios->Num() != Constraint->NumManifoldPoints()) { return nullptr; }
                Constraint->ResetSolverResults();
                for (int32 Index = 0; Index < FrictionRatios->Num(); ++Index)
                {
                    double Ratio = 0;
                    if (!(*FrictionRatios)[Index]->TryGetNumber(Ratio) || !FMath::IsFinite(Ratio) || Ratio < 0 || Ratio > 1) { return nullptr; }
                    Chaos::FManifoldPoint& Point = Constraint->GetManifoldPoint(Index);
                    Point.ShapeAnchorPoints[0] += Chaos::FVec3f(AnchorOffset0);
                    Point.ShapeAnchorPoints[1] += Chaos::FVec3f(AnchorOffset1);
                    Constraint->SetSolverResults(Index, Chaos::FVec3f(0), Chaos::FVec3f(0), Ratio, DeltaSeconds);
                }
                TArray<TSharedPtr<FJsonValue>> SavedPoints;
                for (int32 Index = 0; Index < Constraint->NumSavedManifoldPoints(); ++Index)
                {
                    const Chaos::FSavedManifoldPoint& Point = Constraint->GetSavedManifoldPoint(Index);
                    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
                    Value->SetObjectField(TEXT("shape_point_0_cm"), CaptureVector(FVector(Point.ShapeContactPoints[0])));
                    Value->SetObjectField(TEXT("shape_point_1_cm"), CaptureVector(FVector(Point.ShapeContactPoints[1])));
                    Value->SetNumberField(TEXT("initial_phi_cm"), Point.InitialPhi);
                    SavedPoints.Add(MakeShared<FJsonValueObject>(Value));
                }
                Sample->SetArrayField(TEXT("saved_points"), SavedPoints);
            }
            Constraint->SetLastShapeWorldTransforms(ShapeWorld, BoxWorld);
            if (Constraint->GetManifoldPoints().IsEmpty()) { Constraint->ResetManifold(); }
            const FTransform NextShapeWorld = ShapeLocal * FTransform(NextBodyRotation, NextBodyTranslation);
            const FTransform NextBoxWorld = OtherLocal * FTransform(NextOtherRotation, NextOtherTranslation);
            Constraint->SetShapeWorldTransforms(NextShapeWorld, NextBoxWorld);
            const bool RestoreAllowed = Constraint->GetCanRestoreManifold();
            const bool Restored = RestoreAllowed && Constraint->TryRestoreManifold();
            if (!Restored)
            {
                Constraint->ResetActiveManifoldContacts();
                Chaos::Collisions::UpdateConstraint(*Constraint, NextShapeWorld, NextBoxWorld, DeltaSeconds);
            }
            Sample->SetBoolField(TEXT("restore_allowed"), RestoreAllowed);
            Sample->SetBoolField(TEXT("restored"), Restored);
            Sample->SetArrayField(TEXT("next_points"), CapturePoints(*Constraint, NextShapeWorld, NextBoxWorld));
            Sample->SetArrayField(TEXT("next_gjk_simplex"), CaptureSimplex(*Constraint, NextShapeWorld, NextBoxWorld));
            if (HasHistory)
            {
                Constraint->Activate();
                TArray<TSharedPtr<FJsonValue>> NextHistory;
                for (const Chaos::FManifoldPoint& Point : Constraint->GetManifoldPoints())
                {
                    if (Point.Flags.bDisabled) { continue; }
                    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
                    Value->SetBoolField(TEXT("has_static_friction_anchor"), Point.Flags.bHasStaticFrictionAnchor);
                    Value->SetBoolField(TEXT("initial_contact"), Point.Flags.bInitialContact);
                    Value->SetObjectField(TEXT("shape_anchor_0_cm"), CaptureVector(FVector(Point.ShapeAnchorPoints[0])));
                    Value->SetObjectField(TEXT("shape_anchor_1_cm"), CaptureVector(FVector(Point.ShapeAnchorPoints[1])));
                    Value->SetObjectField(TEXT("world_anchor_delta_cm"), CaptureVector(NextShapeWorld.TransformPositionNoScale(FVector(Point.ShapeAnchorPoints[0])) - NextBoxWorld.TransformPositionNoScale(FVector(Point.ShapeAnchorPoints[1]))));
                    Value->SetNumberField(TEXT("initial_phi_cm"), Point.InitialPhi);
                    NextHistory.Add(MakeShared<FJsonValueObject>(Value));
                }
                Sample->SetArrayField(TEXT("next_history"), NextHistory);
            }
            Constraint->ResetActiveManifoldContacts();
            Chaos::Collisions::UpdateConstraint(*Constraint, NextShapeWorld, NextBoxWorld, DeltaSeconds);
            Sample->SetArrayField(TEXT("fresh_points"), CapturePoints(*Constraint, NextShapeWorld, NextBoxWorld));
        }
        if (AggregateMesh)
        {
            Chaos::FPBDRigidParticleHandle* Dynamic = Particles.CreateDynamicParticles(1)[0];
            Chaos::FGeometryParticleHandle* Static = Particles.CreateStaticParticles(1)[0];
            Dynamic->SetGeometry(Implicit);
            Static->SetGeometry(MeshRoot);
            Dynamic->ShapeInstances()[0]->SetCollisionData(BodyCollisionData);
            if (Static->ShapeInstances().Num() != MeshCollisionData.Num()) { return nullptr; }
            for (int32 Index = 0; Index < MeshCollisionData.Num(); ++Index) { Static->ShapeInstances()[Index]->SetCollisionData(MeshCollisionData[Index]); }
            const FTransform MeshWorld(OtherRotation, OtherTranslation);
            Dynamic->SetX(ShapeWorld.GetLocation());
            Dynamic->SetR(ShapeWorld.GetRotation());
            Dynamic->SetP(ShapeWorld.GetLocation());
            Dynamic->SetQ(ShapeWorld.GetRotation());
            Dynamic->SetCollisionConstraintFlags(Setup->DefaultInstance.bSmoothEdgeCollisions ? static_cast<uint32>(Chaos::ECollisionConstraintFlags::CCF_SmoothEdgeCollisions) : 0);
            Static->SetX(MeshWorld.GetLocation());
            Static->SetR(MeshWorld.GetRotation());
            Dynamic->UpdateWorldSpaceState(ShapeWorld, Chaos::FVec3(CullDistance));
            Static->UpdateWorldSpaceState(MeshWorld, Chaos::FVec3(CullDistance));
            auto& Container = Solver->GetEvolution()->GetCollisionConstraints();
            Container.SetCullDistance(CullDistance);
            const TArray<Chaos::FGeometryParticleHandle*> DynamicParticles{Dynamic};
            const TArray<Chaos::FGeometryParticleHandle*> StaticParticles{Static};
            Chaos::FBasicBroadPhase BroadPhase(nullptr, &DynamicParticles, &StaticParticles);
            Chaos::FBasicCollisionDetector Detector(BroadPhase, Container);
            Detector.DetectCollisions(DeltaSeconds, nullptr);
            TArray<TSharedPtr<FJsonValue>> Contacts;
            for (const Chaos::FPBDCollisionConstraint* Contact : Container.GetConstraints())
            {
                if (Contact->GetParticle0() != Dynamic || Contact->GetParticle1() != Static) { return nullptr; }
                TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
                const int32 Index = Static->ShapeInstances().IndexOfByPredicate([Contact](const auto& Entry) { return Entry.Get() == Contact->GetShape1(); });
                Value->SetNumberField(TEXT("mesh_shape_index"), Index);
                Value->SetBoolField(TEXT("disabled"), Contact->GetDisabled());
                Value->SetNumberField(TEXT("collision_margin_0_cm"), Contact->GetCollisionMargin0());
                Value->SetNumberField(TEXT("collision_margin_1_cm"), Contact->GetCollisionMargin1());
                Value->SetArrayField(TEXT("points"), CapturePoints(*Contact, Contact->GetShapeWorldTransform0(), Contact->GetShapeWorldTransform1()));
                Contacts.Add(MakeShared<FJsonValueObject>(Value));
            }
            Sample->SetArrayField(TEXT("aggregate_contacts"), Contacts);
            Sample->SetNumberField(TEXT("aggregate_shape_count"), Static->ShapeInstances().Num());
            Sample->SetBoolField(TEXT("smooth_edge_collisions"), Setup->DefaultInstance.bSmoothEdgeCollisions);
        }
        Samples.Add(MakeShared<FJsonValueObject>(Sample));
        UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Physics contact probe captured sample %d/%d with %d points."), Samples.Num(), Inputs->Num(), Points.Num());
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetArrayField(TEXT("samples"), Samples);
    return Result;
}

TSharedRef<FJsonObject> CaptureRigHierarchy(const URigHierarchy* Hierarchy)
{
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    TArray<TSharedPtr<FJsonValue>> Elements;
    for (const FRigElementKey& Key : Hierarchy->GetAllKeys())
    {
        TSharedRef<FJsonObject> Element = MakeShared<FJsonObject>();
        Element->SetStringField(TEXT("name"), Key.Name.ToString());
        Element->SetStringField(TEXT("type"), StaticEnum<ERigElementType>()->GetNameStringByValue(static_cast<int64>(Key.Type)));
        if (Key.Type == ERigElementType::Curve)
        {
            Element->SetBoolField(TEXT("value_set"), Hierarchy->IsCurveValueSet(Key));
            Element->SetNumberField(TEXT("value"), Hierarchy->GetCurveValue(Key));
        }
        Elements.Add(MakeShared<FJsonValueObject>(Element));
    }
    Value->SetArrayField(TEXT("elements"), Elements);
    Value->SetNumberField(TEXT("element_count"), Hierarchy->Num());
    TArray<TSharedPtr<FJsonValue>> Bones;
    for (const FRigElementKey& Key : Hierarchy->GetBoneKeys())
    {
        TSharedRef<FJsonObject> Bone = MakeShared<FJsonObject>();
        Bone->SetStringField(TEXT("name"), Key.Name.ToString());
        Bone->SetStringField(TEXT("parent"), Hierarchy->GetFirstParent(Key).Name.ToString());
        Bone->SetObjectField(TEXT("initial_global_transform"), CaptureTransform(Hierarchy->GetInitialGlobalTransform(Key)));
        Bone->SetObjectField(TEXT("initial_local_transform"), CaptureTransform(Hierarchy->GetInitialLocalTransform(Key)));
        Bones.Add(MakeShared<FJsonValueObject>(Bone));
    }
    Value->SetArrayField(TEXT("bones"), Bones);
    return Value;
}

TSharedRef<FJsonObject> CaptureSkeletalMesh(const USkeletalMesh* Mesh)
{
    const FReferenceSkeleton& ReferenceSkeleton = Mesh->GetRefSkeleton();
    const TArray<FTransform>& ReferencePose = ReferenceSkeleton.GetRefBonePose();
    TArray<FTransform> ComponentSpaceTransforms;
    FAnimationRuntime::FillUpComponentSpaceTransforms(ReferenceSkeleton, ReferencePose, ComponentSpaceTransforms);
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
    Value->SetStringField(TEXT("skeleton_path"), ObjectPath(Mesh->GetSkeleton()));
    Value->SetNumberField(TEXT("raw_bone_count"), ReferenceSkeleton.GetRawBoneNum());
    Value->SetNumberField(TEXT("bone_count"), ReferenceSkeleton.GetNum());

    TArray<TSharedPtr<FJsonValue>> Bones;
    for (int32 BoneIndex = 0; BoneIndex < ReferenceSkeleton.GetNum(); ++BoneIndex)
    {
        TSharedRef<FJsonObject> Bone = MakeShared<FJsonObject>();
        Bone->SetNumberField(TEXT("index"), BoneIndex);
        Bone->SetStringField(TEXT("name"), ReferenceSkeleton.GetBoneName(BoneIndex).ToString());
        Bone->SetNumberField(TEXT("parent_index"), ReferenceSkeleton.GetParentIndex(BoneIndex));
        Bone->SetBoolField(TEXT("virtual"), BoneIndex >= ReferenceSkeleton.GetRawBoneNum());
        Bone->SetObjectField(TEXT("local_transform"), CaptureTransform(ReferencePose[BoneIndex]));
        Bone->SetObjectField(TEXT("component_transform"), CaptureTransform(ComponentSpaceTransforms[BoneIndex]));
        Bones.Add(MakeShared<FJsonValueObject>(Bone));
    }
    Value->SetArrayField(TEXT("bones"), Bones);

    TArray<TSharedPtr<FJsonValue>> VirtualBones;
    if (const USkeleton* Skeleton = Mesh->GetSkeleton())
    {
        for (const FVirtualBone& VirtualBone : Skeleton->GetVirtualBones())
        {
            TSharedRef<FJsonObject> Bone = MakeShared<FJsonObject>();
            Bone->SetStringField(TEXT("source_bone"), VirtualBone.SourceBoneName.ToString());
            Bone->SetStringField(TEXT("target_bone"), VirtualBone.TargetBoneName.ToString());
            Bone->SetStringField(TEXT("virtual_bone"), VirtualBone.VirtualBoneName.ToString());
            VirtualBones.Add(MakeShared<FJsonValueObject>(Bone));
        }
    }
    Value->SetArrayField(TEXT("virtual_bones"), VirtualBones);
    return Value;
}
}

TSharedPtr<FJsonObject> CaptureBlendSpaceProbe(UObject* Asset, const FString& RequestPath)
{
    UBlendSpace* BlendSpace = Cast<UBlendSpace>(Asset);
    FString RequestText;
    TSharedPtr<FJsonObject> Request;
    if (BlendSpace == nullptr || !FFileHelper::LoadFileToString(RequestText, *RequestPath) ||
        !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(RequestText), Request))
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("BlendSpace probe asset or request is invalid."));
        return nullptr;
    }
    const TArray<TSharedPtr<FJsonValue>>* Frames;
    double StartPosition;
    bool Looped;
    if (!Request->TryGetArrayField(TEXT("frames"), Frames) || Frames->IsEmpty() || Frames->Num() > 600 ||
        !Request->TryGetNumberField(TEXT("start_position"), StartPosition) || !FMath::IsFinite(StartPosition) ||
        StartPosition < 0.0 || StartPosition > 1.0 || !Request->TryGetBoolField(TEXT("looped"), Looped))
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("BlendSpace probe timeline is invalid."));
        return nullptr;
    }
    float NormalizedTime = static_cast<float>(StartPosition);
    FBlendFilter Filter;
    BlendSpace->InitializeFilter(&Filter);
    TArray<FBlendSampleData> Samples;
    FMarkerTickRecord MarkerRecord;
    FDeltaTimeRecord DeltaRecord;
    FMarkerSyncAnimPosition LastMarkerPosition;
    int32 TriangulationIndex = INDEX_NONE;
    const TArray<FName> EmptyMarkers;
    const TArray<FName>* Markers = BlendSpace->GetUniqueMarkerNames();
    TArray<TSharedPtr<FJsonValue>> Results;
    for (int32 Index = 0; Index < Frames->Num(); ++Index)
    {
        const TSharedPtr<FJsonObject>* Input;
        double X, Y, DeltaSeconds, PlayRate;
        if (!(*Frames)[Index]->TryGetObject(Input) ||
            !(*Input)->TryGetNumberField(TEXT("x"), X) || !FMath::IsFinite(X) ||
            !(*Input)->TryGetNumberField(TEXT("y"), Y) || !FMath::IsFinite(Y) ||
            !(*Input)->TryGetNumberField(TEXT("delta_seconds"), DeltaSeconds) || !FMath::IsFinite(DeltaSeconds) ||
            DeltaSeconds < 0.0 || DeltaSeconds > 0.25 ||
            !(*Input)->TryGetNumberField(TEXT("play_rate"), PlayRate) || !FMath::IsFinite(PlayRate))
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("BlendSpace probe frame %d is invalid."), Index);
            return nullptr;
        }
        FAnimTickRecord TickRecord(BlendSpace, FVector(X, Y, 0.0), Samples, Filter, Looped,
            static_cast<float>(PlayRate), false, false, 1.0f, NormalizedTime, MarkerRecord);
        TickRecord.DeltaTimeRecord = &DeltaRecord;
        TickRecord.bCanUseMarkerSync = Markers != nullptr && !Markers->IsEmpty();
        TickRecord.BlendSpace.TriangulationIndex = TriangulationIndex;
        FAnimAssetTickContext Context(static_cast<float>(DeltaSeconds), ERootMotionMode::NoRootMotionExtraction,
            true, Markers != nullptr ? *Markers : EmptyMarkers);
        if (LastMarkerPosition.IsValid())
        {
            Context.MarkerTickContext.SetMarkerSyncStartPosition(LastMarkerPosition);
        }
        FAnimNotifyQueue NotifyQueue;
        BlendSpace->TickAssetPlayer(TickRecord, NotifyQueue, Context);
        TriangulationIndex = TickRecord.BlendSpace.TriangulationIndex;
        LastMarkerPosition = Context.MarkerTickContext.GetMarkerSyncEndPosition();
        TSharedRef<FJsonObject> Frame = MakeShared<FJsonObject>();
        Frame->SetNumberField(TEXT("frame"), Index);
        Frame->SetObjectField(TEXT("input"), Input->ToSharedRef());
        Frame->SetObjectField(TEXT("filtered_position"), CaptureVector(Filter.GetFilterLastOutput()));
        Frame->SetNumberField(TEXT("normalized_time"), NormalizedTime);
        Frame->SetNumberField(TEXT("leader_delta"), Context.GetLeaderDelta());
        Frame->SetStringField(TEXT("previous_marker"), LastMarkerPosition.PreviousMarkerName.ToString());
        Frame->SetStringField(TEXT("next_marker"), LastMarkerPosition.NextMarkerName.ToString());
        Frame->SetNumberField(TEXT("marker_alpha"), LastMarkerPosition.PositionBetweenMarkers);
        TArray<TSharedPtr<FJsonValue>> SampleValues;
        for (const FBlendSampleData& Sample : Samples)
        {
            TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
            Value->SetNumberField(TEXT("sample_index"), Sample.SampleDataIndex);
            Value->SetStringField(TEXT("animation"), ObjectPath(Sample.Animation));
            Value->SetNumberField(TEXT("weight"), Sample.GetClampedWeight());
            Value->SetNumberField(TEXT("previous_time"), Sample.PreviousTime);
            Value->SetNumberField(TEXT("time"), Sample.Time);
            Value->SetNumberField(TEXT("play_rate"), Sample.SamplePlayRate);
            SampleValues.Add(MakeShared<FJsonValueObject>(Value));
        }
        Frame->SetArrayField(TEXT("samples"), SampleValues);
        Results.Add(MakeShared<FJsonValueObject>(Frame));
        if (Index % 60 == 0)
        {
            UE_LOG(LogAlsGraphSourceReader, Display, TEXT("BlendSpace probe frame %d/%d."), Index + 1, Frames->Num());
        }
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetStringField(TEXT("format"), TEXT("ue_blendspace_tick_probe_v1"));
    Result->SetNumberField(TEXT("start_position"), StartPosition);
    Result->SetBoolField(TEXT("looped"), Looped);
    Result->SetArrayField(TEXT("frames"), Results);
    return Result;
}

struct FSequencePoseAccess : UAnimSequence
{
    static bool CanUseRawData(const UAnimSequence& Sequence) { return (Sequence.*&FSequencePoseAccess::CanEvaluateRawAnimationData)(); }
    static bool UsesRawData(const UAnimSequence& Sequence, const FBoneContainer& Bones, const FAnimExtractContext& Context)
    {
        return (Sequence.*&FSequencePoseAccess::ShouldUseRawDataForPoseExtraction)(Bones, Context);
    }
};

TSharedRef<FJsonObject> CaptureMovieSceneFloatChannel(const FMovieSceneFloatChannel& Channel)
{
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    const FFrameRate Rate = Channel.GetTickResolution();
    Result->SetNumberField(TEXT("tick_numerator"), Rate.Numerator);
    Result->SetNumberField(TEXT("tick_denominator"), Rate.Denominator);
    Result->SetNumberField(TEXT("pre_infinity"), Channel.PreInfinityExtrap.GetValue());
    Result->SetNumberField(TEXT("post_infinity"), Channel.PostInfinityExtrap.GetValue());
    const TOptional<float> Default = Channel.GetDefault();
    Result->SetField(TEXT("default"), Default.IsSet() ? StaticCastSharedRef<FJsonValue>(MakeShared<FJsonValueNumber>(Default.GetValue())) : StaticCastSharedRef<FJsonValue>(MakeShared<FJsonValueNull>()));
    const TArrayView<const FFrameNumber> Times = Channel.GetTimes();
    const TArrayView<const FMovieSceneFloatValue> Values = Channel.GetValues();
    TArray<TSharedPtr<FJsonValue>> Keys;
    for (int32 Index = 0; Index < Values.Num(); ++Index)
    {
        const FMovieSceneFloatValue& Value = Values[Index];
        TSharedRef<FJsonObject> Key = MakeShared<FJsonObject>();
        Key->SetNumberField(TEXT("frame"), Times[Index].Value);
        Key->SetNumberField(TEXT("value"), Value.Value);
        Key->SetNumberField(TEXT("interp_mode"), Value.InterpMode.GetValue());
        Key->SetNumberField(TEXT("tangent_mode"), Value.TangentMode.GetValue());
        Key->SetNumberField(TEXT("arrive_tangent"), Value.Tangent.ArriveTangent);
        Key->SetNumberField(TEXT("leave_tangent"), Value.Tangent.LeaveTangent);
        Key->SetNumberField(TEXT("weight_mode"), Value.Tangent.TangentWeightMode.GetValue());
        Key->SetNumberField(TEXT("arrive_weight"), Value.Tangent.ArriveTangentWeight);
        Key->SetNumberField(TEXT("leave_weight"), Value.Tangent.LeaveTangentWeight);
        Keys.Add(MakeShared<FJsonValueObject>(Key));
    }
    Result->SetArrayField(TEXT("keys"), Keys);
    return Result;
}

TSharedPtr<FJsonObject> CaptureSequenceSourceChannels(const UAnimSequence& Sequence)
{
    UObject* Model = Sequence.GetDataModelInterface().GetObject();
    if (Model == nullptr) { return nullptr; }
    TArray<UObject*> Objects;
    GetObjectsWithOuter(Model, Objects, EGetObjectsFlags::IncludeNestedObjects);
    Objects.Sort([](const UObject& Left, const UObject& Right) { return Left.GetPathName() < Right.GetPathName(); });
    TArray<TSharedPtr<FJsonValue>> Sections;
    for (UObject* Object : Objects)
    {
        const UMovieSceneControlRigParameterSection* Section = Cast<UMovieSceneControlRigParameterSection>(Object);
        if (Section == nullptr) { continue; }
        TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
        Value->SetStringField(TEXT("path"), ObjectPath(Section));
        Value->SetBoolField(TEXT("active"), Section->IsActive());
        TArray<TSharedPtr<FJsonValue>> Scalars;
        for (const FScalarParameterNameAndCurve& Parameter : Section->GetScalarParameterNamesAndCurves())
        {
            TSharedRef<FJsonObject> Scalar = MakeShared<FJsonObject>();
            Scalar->SetStringField(TEXT("name"), Parameter.ParameterName.ToString());
            Scalar->SetObjectField(TEXT("channel"), CaptureMovieSceneFloatChannel(Parameter.ParameterCurve));
            Scalars.Add(MakeShared<FJsonValueObject>(Scalar));
        }
        Value->SetArrayField(TEXT("scalars"), Scalars);
        TArray<TSharedPtr<FJsonValue>> Transforms;
        for (const FTransformParameterNameAndCurves& Parameter : Section->GetTransformParameterNamesAndCurves())
        {
            TSharedRef<FJsonObject> Transform = MakeShared<FJsonObject>();
            Transform->SetStringField(TEXT("name"), Parameter.ParameterName.ToString());
            TArray<TSharedPtr<FJsonValue>> Translation, Rotation, Scale;
            for (int32 Axis = 0; Axis < 3; ++Axis)
            {
                Translation.Add(MakeShared<FJsonValueObject>(CaptureMovieSceneFloatChannel(Parameter.Translation[Axis])));
                Rotation.Add(MakeShared<FJsonValueObject>(CaptureMovieSceneFloatChannel(Parameter.Rotation[Axis])));
                Scale.Add(MakeShared<FJsonValueObject>(CaptureMovieSceneFloatChannel(Parameter.Scale[Axis])));
            }
            Transform->SetArrayField(TEXT("translation"), Translation);
            Transform->SetArrayField(TEXT("rotation"), Rotation);
            Transform->SetArrayField(TEXT("scale"), Scale);
            Transforms.Add(MakeShared<FJsonValueObject>(Transform));
        }
        Value->SetArrayField(TEXT("transforms"), Transforms);
        Sections.Add(MakeShared<FJsonValueObject>(Value));
    }
    if (Sections.IsEmpty()) { return nullptr; }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetStringField(TEXT("model"), ObjectPath(Model));
    Result->SetArrayField(TEXT("sections"), Sections);
    return Result;
}

TSharedPtr<FJsonObject> CaptureSequenceRigStages(const UAnimSequence& Sequence, double Time, const FBoneContainer& RequiredBones)
{
    UObject* Model = Sequence.GetDataModelInterface().GetObject();
    if (Model == nullptr) { return nullptr; }
    TArray<UObject*> Objects;
    GetObjectsWithOuter(Model, Objects, EGetObjectsFlags::IncludeNestedObjects);
    TSharedPtr<FJsonObject> Result;
    for (UObject* Object : Objects)
    {
        const UMovieSceneControlRigParameterSection* Section = Cast<UMovieSceneControlRigParameterSection>(Object);
        if (Section == nullptr || !Section->IsActive()) { continue; }
        UControlRig* Rig = Section->GetControlRig();
        URigHierarchy* Hierarchy = Rig != nullptr ? Rig->GetHierarchy() : nullptr;
        if (Hierarchy == nullptr || Result.IsValid()) { return nullptr; }
        Result = MakeShared<FJsonObject>();
        Result->SetStringField(TEXT("section"), ObjectPath(Section));
        const UFKControlRig* FK = Cast<UFKControlRig>(Rig);
        if (FK != nullptr) { Result->SetNumberField(TEXT("apply_mode"), static_cast<int32>(FK->GetApplyMode())); }
        const IAnimationDataModel* DataModel = Sequence.GetDataModelInterface().GetInterface();
        const UMovieScene* Scene = Section->GetTypedOuter<UMovieScene>();
        if (DataModel == nullptr || Scene == nullptr) { return nullptr; }
        const FFrameTime SampleTime = FFrameRate::TransformTime(DataModel->GetFrameRate().AsFrameTime(Time), DataModel->GetFrameRate(), Scene->GetTickResolution());
        const float Alpha = SampleTime.GetSubFrame();
        const auto ChannelValue = [DataModel](const FMovieSceneFloatChannel& Channel, FFrameNumber Frame, bool bPreferDefault, float Fallback)
        {
            if (bPreferDefault && Channel.GetDefault().IsSet()) { return Channel.GetDefault().GetValue(); }
            if (!bPreferDefault && Channel.GetValues().Num() == DataModel->GetNumberOfKeys()) { return Channel.GetValues()[Frame.Value].Value; }
            float Value = Fallback;
            Channel.Evaluate(Frame, Value);
            return Value;
        };
        TMap<FName, FTransform> ChannelControls;
        for (const FTransformParameterNameAndCurves& Parameter : Section->GetTransformParameterNamesAndCurves())
        {
            const auto EvaluateEuler = [&](FFrameNumber Frame, bool bPreferDefault)
            {
                return FEulerTransform(FRotator(ChannelValue(Parameter.Rotation[1], Frame, bPreferDefault, 0.f), ChannelValue(Parameter.Rotation[2], Frame, bPreferDefault, 0.f), ChannelValue(Parameter.Rotation[0], Frame, bPreferDefault, 0.f)),
                    FVector(ChannelValue(Parameter.Translation[0], Frame, bPreferDefault, 0.f), ChannelValue(Parameter.Translation[1], Frame, bPreferDefault, 0.f), ChannelValue(Parameter.Translation[2], Frame, bPreferDefault, 0.f)),
                    FVector(ChannelValue(Parameter.Scale[0], Frame, bPreferDefault, 1.f), ChannelValue(Parameter.Scale[1], Frame, bPreferDefault, 1.f), ChannelValue(Parameter.Scale[2], Frame, bPreferDefault, 1.f)));
            };
            FEulerTransform Euler;
            if (FMath::IsNearlyZero(Alpha)) { Euler = EvaluateEuler(SampleTime.FrameNumber, false); }
            else
            {
                const FTransform From = EvaluateEuler(SampleTime.FloorToFrame(), true).ToFTransform();
                const FTransform To = EvaluateEuler(SampleTime.CeilToFrame(), true).ToFTransform();
                FTransform Blended;
                Blended.Blend(From, To, Alpha);
                Euler = FEulerTransform(Blended);
            }
            ChannelControls.Add(Parameter.ParameterName, FRigControlValue::Make<FRigControlValue::FEulerTransform_Float>(FRigControlValue::FEulerTransform_Float(Euler)).GetAsTransform(ERigControlType::EulerTransform, ERigControlAxis::X));
        }
        TArray<TSharedPtr<FJsonValue>> Bones;
        Hierarchy->ForEach<FRigBoneElement>([&](const FRigBoneElement* Bone)
        {
            const FRigElementKey Key = Bone->GetKey();
            TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
            Value->SetStringField(TEXT("name"), Key.Name.ToString());
            Value->SetStringField(TEXT("parent"), Hierarchy->GetFirstParent(Key).Name.ToString());
            Value->SetObjectField(TEXT("bone_local"), CaptureTransform(Hierarchy->GetLocalTransform(Key)));
            Value->SetObjectField(TEXT("bone_initial_local"), CaptureTransform(Hierarchy->GetInitialLocalTransform(Key)));
            const FRigElementKey ControlKey(UFKControlRig::GetControlName(Key.Name, Key.Type), ERigElementType::Control);
            if (const FTransform* Replayed = ChannelControls.Find(ControlKey.Name))
            {
                Value->SetObjectField(TEXT("native_channel_control"), CaptureTransform(*Replayed));
                const FReferenceSkeleton& Reference = Sequence.GetSkeleton()->GetReferenceSkeleton();
                const int32 SkeletonIndex = Reference.FindBoneIndex(Key.Name);
                const FCompactPoseBoneIndex CompactIndex = RequiredBones.GetCompactPoseIndexFromSkeletonPoseIndex(FSkeletonPoseBoneIndex(SkeletonIndex));
                FTransform ReplayedBone = Hierarchy->GetLocalTransform(Key);
                ReplayedBone.NormalizeRotation();
                Value->SetObjectField(TEXT("normalized_bone"), CaptureTransform(ReplayedBone));
                Value->SetNumberField(TEXT("retarget_mode"), Sequence.GetSkeleton()->GetBoneTranslationRetargetingMode(SkeletonIndex));
                FAnimationRuntime::RetargetBoneTransform(Sequence.GetSkeleton(), Sequence.GetRetargetTransformsSourceName(), Sequence.GetRetargetTransforms(), ReplayedBone, SkeletonIndex, CompactIndex, RequiredBones, false);
                Value->SetObjectField(TEXT("retargeted_bone"), CaptureTransform(ReplayedBone));
            }
            FRigControlElement* Control = Hierarchy->Find<FRigControlElement>(ControlKey);
            if (Control != nullptr)
            {
                Value->SetObjectField(TEXT("control_local"), CaptureTransform(Hierarchy->GetLocalTransform(ControlKey)));
                Value->SetObjectField(TEXT("control_offset_local"), CaptureTransform(Hierarchy->GetControlOffsetTransform(Control, ERigTransformType::InitialLocal)));
            }
            Bones.Add(MakeShared<FJsonValueObject>(Value));
            return true;
        });
        Result->SetArrayField(TEXT("bones"), Bones);
    }
    return Result;
}

void CollectAnimationReferences(const FProperty* Property, const void* Data, const FString& Path, TArray<TSharedPtr<FJsonValue>>& References, TArray<UAnimationAsset*>& Assets)
{
    if (const FObjectPropertyBase* ObjectProperty = CastField<FObjectPropertyBase>(Property))
    {
        if (UAnimationAsset* Animation = Cast<UAnimationAsset>(ObjectProperty->GetObjectPropertyValue(Data)))
        {
            TSharedRef<FJsonObject> Reference = MakeShared<FJsonObject>();
            Reference->SetStringField(TEXT("property"), Path);
            Reference->SetStringField(TEXT("asset"), Animation->GetPathName());
            References.Add(MakeShared<FJsonValueObject>(Reference));
            Assets.AddUnique(Animation);
        }
    }
    else if (const FStructProperty* StructProperty = CastField<FStructProperty>(Property))
    {
        for (TFieldIterator<FProperty> Field(StructProperty->Struct, EFieldIterationFlags::IncludeSuper); Field; ++Field)
        {
            for (int32 Index = 0; Index < Field->ArrayDim; ++Index)
            {
                CollectAnimationReferences(*Field, Field->ContainerPtrToValuePtr<void>(Data, Index), Path + TEXT(".") + Field->GetName() + FString::Printf(TEXT("[%d]"), Index), References, Assets);
            }
        }
    }

}

TSharedPtr<FJsonObject> CaptureAnimationAttributeProbe(const FString& RequestPath)
{
    FString Text;
    TSharedPtr<FJsonObject> Request;
    const TArray<TSharedPtr<FJsonValue>>* BlueprintPaths = nullptr;
    if (!FFileHelper::LoadFileToString(Text, *RequestPath) || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) ||
        !Request->TryGetArrayField(TEXT("animation_blueprints"), BlueprintPaths) || BlueprintPaths->IsEmpty() || BlueprintPaths->Num() > 64) { return nullptr; }
    TArray<UAnimationAsset*> Assets;
    TArray<TSharedPtr<FJsonValue>> Blueprints;
    TSet<FString> Seen;
    for (const TSharedPtr<FJsonValue>& Value : *BlueprintPaths)
    {
        FString Path;
        if (!Value->TryGetString(Path) || Path.IsEmpty() || Seen.Contains(Path)) { return nullptr; }
        Seen.Add(Path);
        UBlueprint* Blueprint = LoadObject<UBlueprint>(nullptr, *Path);
        const IAnimClassInterface* Interface = Blueprint == nullptr ? nullptr : IAnimClassInterface::GetFromClass(Blueprint->GeneratedClass);
        if (Interface == nullptr) { return nullptr; }
        TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
        Row->SetStringField(TEXT("asset"), Path);
        TArray<TSharedPtr<FJsonValue>> Nodes;
        for (const FStructProperty* Property : Interface->GetAnimNodeProperties())
        {
            TSharedRef<FJsonObject> Node = MakeShared<FJsonObject>();
            Node->SetStringField(TEXT("property"), Property->GetName());
            Node->SetStringField(TEXT("type"), Property->Struct->GetPathName());
            TArray<TSharedPtr<FJsonValue>> References;
            CollectAnimationReferences(Property, Property->ContainerPtrToValuePtr<void>(Blueprint->GeneratedClass->GetDefaultObject()), Property->GetName(), References, Assets);
            Node->SetArrayField(TEXT("references"), References);
            Nodes.Add(MakeShared<FJsonValueObject>(Node));
        }
        Row->SetArrayField(TEXT("nodes"), Nodes);
        Blueprints.Add(MakeShared<FJsonValueObject>(Row));
    }
    TArray<TSharedPtr<FJsonValue>> AssetRows;
    for (int32 Index = 0; Index < Assets.Num(); ++Index)
    {
        if (Assets.Num() > 4096) { return nullptr; }
        UAnimationAsset* Animation = Assets[Index];
        UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Animation attribute closure asset %d: %s"), Index, *Animation->GetPathName());
        TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
        Row->SetStringField(TEXT("asset"), Animation->GetPathName());
        Row->SetStringField(TEXT("type"), Animation->GetClass()->GetPathName());
        TArray<UAnimationAsset*> Dependencies;
        Animation->GetAllAnimationSequencesReferred(Dependencies, false);
        TArray<TSharedPtr<FJsonValue>> DependencyPaths;
        for (UAnimationAsset* Dependency : Dependencies)
        {
            if (Dependency == nullptr) { return nullptr; }
            DependencyPaths.Add(MakeShared<FJsonValueString>(Dependency->GetPathName()));
            Assets.AddUnique(Dependency);
        }
        Row->SetArrayField(TEXT("dependencies"), DependencyPaths);
        if (UAnimSequence* Sequence = Cast<UAnimSequence>(Animation))
        {
            const IAnimationDataModel* Model = Sequence->GetDataModel();
            const FMapProperty* Baked = FindFProperty<FMapProperty>(Sequence->GetClass(), TEXT("AttributeCurves"));
            if (Model == nullptr || Baked == nullptr || Sequence->GetSkeleton() == nullptr) { return nullptr; }
            Row->SetNumberField(TEXT("model_attributes"), Model->GetNumberOfAttributes());
            Row->SetNumberField(TEXT("baked_attributes"), FScriptMapHelper(Baked, Baked->ContainerPtrToValuePtr<void>(Sequence)).Num());
            Row->SetBoolField(TEXT("has_root_motion"), Sequence->HasRootMotion());
            TArray<TSharedPtr<FJsonValue>> Identifiers;
            for (const FAnimatedBoneAttribute& Attribute : Model->GetAttributes())
            {
                TSharedRef<FJsonObject> Identifier = MakeShared<FJsonObject>();
                Identifier->SetStringField(TEXT("name"), Attribute.Identifier.GetName().ToString());
                Identifier->SetStringField(TEXT("bone"), Attribute.Identifier.GetBoneName().ToString());
                Identifier->SetStringField(TEXT("type"), Attribute.Identifier.GetScriptStructPath().ToString());
                Identifiers.Add(MakeShared<FJsonValueObject>(Identifier));
            }
            Row->SetArrayField(TEXT("model_identifiers"), Identifiers);
            Sequence->CacheDerivedDataForCurrentPlatform();
            FBoneContainer Bones;
            TArray<FBoneIndexType> BoneIndices;
            for (int32 Bone = 0; Bone < Sequence->GetSkeleton()->GetReferenceSkeleton().GetNum(); ++Bone) { BoneIndices.Add(Bone); }
            Bones.InitializeTo(BoneIndices, UE::Anim::FCurveFilterSettings(), *Sequence->GetSkeleton());
            TArray<TSharedPtr<FJsonValue>> Samples;
            for (double Fraction : {0.0, 0.5, 1.0})
            {
                FMemMark Mark(FMemStack::Get());
                FCompactPose Pose;
                Pose.SetBoneContainer(&Bones);
                FBlendedCurve Curve;
                Curve.InitFrom(Bones);
                UE::Anim::FStackAttributeContainer Attributes;
                FAnimationPoseData PoseData(Pose, Curve, Attributes);
                const FAnimExtractContext Context(Fraction * Sequence->GetPlayLength(), false);
                TSharedRef<FJsonObject> Sample = MakeShared<FJsonObject>();
                Sample->SetNumberField(TEXT("time"), Context.CurrentTime);
                Sample->SetBoolField(TEXT("raw_data"), FSequencePoseAccess::UsesRawData(*Sequence, Bones, Context));
                Sequence->GetAnimationPose(PoseData, Context);
                Sample->SetNumberField(TEXT("attribute_count"), Attributes.Num());
                TArray<TSharedPtr<FJsonValue>> Values;
                const auto& Types = Attributes.GetUniqueTypes();
                for (int32 TypeIndex = 0; TypeIndex < Types.Num(); ++TypeIndex)
                {
                    const UScriptStruct* Type = Types[TypeIndex].Get();
                    if (Type == nullptr) { return nullptr; }
                    const auto& Keys = Attributes.GetKeys(TypeIndex);
                    const auto& AttributeValues = Attributes.GetValues(TypeIndex);
                    for (int32 KeyIndex = 0; KeyIndex < Keys.Num(); ++KeyIndex)
                    {
                        TSharedRef<FJsonObject> Item = MakeShared<FJsonObject>();
                        Item->SetStringField(TEXT("name"), Keys[KeyIndex].GetName().ToString());
                        Item->SetStringField(TEXT("type"), Type->GetPathName());
                        Item->SetNumberField(TEXT("bone_index"), Keys[KeyIndex].GetIndex());
                        FString Exported;
                        Type->ExportText(Exported, AttributeValues[KeyIndex].GetPtr<void>(), nullptr, nullptr, PPF_None, nullptr);
                        Item->SetStringField(TEXT("value"), Exported);
                        Values.Add(MakeShared<FJsonValueObject>(Item));
                    }
                }
                Sample->SetArrayField(TEXT("attributes"), Values);
                Samples.Add(MakeShared<FJsonValueObject>(Sample));
            }
            Row->SetArrayField(TEXT("samples"), Samples);
        }
        AssetRows.Add(MakeShared<FJsonValueObject>(Row));
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetBoolField(TEXT("root_motion_provider_available"), UE::Anim::IAnimRootMotionProvider::IsAvailable());
    Result->SetArrayField(TEXT("blueprints"), Blueprints);
    Result->SetArrayField(TEXT("assets"), AssetRows);
    return Result;
}

TSharedPtr<FJsonObject> CaptureSequencePoseProbe(UObject* Asset, const FString& RequestPath)
{
    UAnimSequence* Sequence = Cast<UAnimSequence>(Asset);
    FString Text;
    TSharedPtr<FJsonObject> Request;
    if (Sequence == nullptr || Sequence->GetSkeleton() == nullptr ||
        !FFileHelper::LoadFileToString(Text, *RequestPath) ||
        !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request))
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Sequence pose probe asset or request is invalid."));
        return nullptr;
    }
    const TArray<TSharedPtr<FJsonValue>>* Times;
    if (!Request->TryGetArrayField(TEXT("times"), Times) || Times->IsEmpty() || Times->Num() > 600)
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Sequence pose probe times are invalid."));
        return nullptr;
    }
    bool bCaptureSourceChannels = false;
    if (Request->HasField(TEXT("capture_source_channels")) &&
        (Request->TryGetField(TEXT("capture_source_channels"))->Type != EJson::Boolean || !Request->TryGetBoolField(TEXT("capture_source_channels"), bCaptureSourceChannels))) { return nullptr; }
    TSharedPtr<FJsonObject> SourceChannels;
    if (bCaptureSourceChannels)
    {
        SourceChannels = CaptureSequenceSourceChannels(*Sequence);
        if (!SourceChannels.IsValid())
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Source channel capture requires an active MovieScene parameter section."));
            return nullptr;
        }
    }
    bool bRootTrackOnly = false;
    Request->TryGetBoolField(TEXT("root_track_only"), bRootTrackOnly);
    bool bAnimationPose = false;
    Request->TryGetBoolField(TEXT("animation_pose"), bAnimationPose);
    bool bRawAbsolutePose = false;
    if (Request->HasField(TEXT("raw_absolute_pose")) &&
        (Request->TryGetField(TEXT("raw_absolute_pose"))->Type != EJson::Boolean || !Request->TryGetBoolField(TEXT("raw_absolute_pose"), bRawAbsolutePose))) { return nullptr; }
    if (bRawAbsolutePose && (!bAnimationPose || !FSequencePoseAccess::CanUseRawData(*Sequence))) { return nullptr; }
    bool bRawAdditivePose = false;
    if (Request->HasField(TEXT("raw_additive_pose")) &&
        (Request->TryGetField(TEXT("raw_additive_pose"))->Type != EJson::Boolean || !Request->TryGetBoolField(TEXT("raw_additive_pose"), bRawAdditivePose))) { return nullptr; }
    if (bRawAdditivePose && (!bAnimationPose || bRawAbsolutePose || !Sequence->IsValidAdditive() || !FSequencePoseAccess::CanUseRawData(*Sequence))) { return nullptr; }
    bool bCompressedTracks = false;
    if (Request->HasField(TEXT("compressed_tracks")) &&
        (Request->TryGetField(TEXT("compressed_tracks"))->Type != EJson::Boolean || !Request->TryGetBoolField(TEXT("compressed_tracks"), bCompressedTracks))) { return nullptr; }
    if (bCompressedTracks && (bAnimationPose || bRootTrackOnly)) { return nullptr; }
    bool bCaptureSourceStages = false;
    if (Request->HasField(TEXT("capture_source_stages")) &&
        (Request->TryGetField(TEXT("capture_source_stages"))->Type != EJson::Boolean || !Request->TryGetBoolField(TEXT("capture_source_stages"), bCaptureSourceStages))) { return nullptr; }
    if (bCaptureSourceStages && (!bAnimationPose || bRawAdditivePose)) { return nullptr; }
    bool bPrepareCompressed = false;
    if (Request->HasField(TEXT("prepare_compressed")) &&
        (Request->TryGetField(TEXT("prepare_compressed"))->Type != EJson::Boolean || !Request->TryGetBoolField(TEXT("prepare_compressed"), bPrepareCompressed))) { return nullptr; }
    if (bCompressedTracks && !bPrepareCompressed) { return nullptr; }
    if (bRawAbsolutePose && bPrepareCompressed) { return nullptr; }
    if (bPrepareCompressed) { Sequence->CacheDerivedDataForCurrentPlatform(); }
    TSharedPtr<FJsonObject> Compression;
    if (bCompressedTracks)
    {
        const auto Scoped = Sequence->GetCompressedData(FAnimExtractContext(0.0, false));
        const FCompressedAnimSequence& Data = Scoped.Get();
        if (!Data.IsValid(Sequence) || Data.CompressedDataStructure == nullptr || Data.BoneCompressionCodec == nullptr || Data.CurveCompressionCodec == nullptr) { return nullptr; }
        Compression = MakeShared<FJsonObject>();
        Compression->SetStringField(TEXT("bone_codec"), ObjectPath(Data.BoneCompressionCodec.Get()));
        Compression->SetStringField(TEXT("curve_codec"), ObjectPath(Data.CurveCompressionCodec.Get()));
        Compression->SetNumberField(TEXT("key_count"), Data.CompressedDataStructure->CompressedNumberOfKeys);
        Compression->SetNumberField(TEXT("curve_bytes"), Data.CompressedCurveByteStream.Num());
        TArray<TSharedPtr<FJsonValue>> CurveNames;
        for (const FAnimCompressedCurveIndexedName& Name : Data.IndexedCurveNames)
        {
            TSharedRef<FJsonObject> Item = MakeShared<FJsonObject>();
            Item->SetStringField(TEXT("name"), Name.CurveName.ToString());
            Item->SetNumberField(TEXT("sorted_index"), Name.CurveIndex);
            CurveNames.Add(MakeShared<FJsonValueObject>(Item));
        }
        Compression->SetArrayField(TEXT("indexed_curve_names"), CurveNames);
        Compression->SetStringField(TEXT("bone_stream_base64"), FBase64::Encode(Data.CompressedByteStream));
        Compression->SetStringField(TEXT("curve_stream_base64"), FBase64::Encode(Data.CompressedCurveByteStream));
        Compression->SetNumberField(TEXT("sampling_rate_numerator"), Sequence->GetSamplingFrameRate().Numerator);
        Compression->SetNumberField(TEXT("sampling_rate_denominator"), Sequence->GetSamplingFrameRate().Denominator);
        TArray<TSharedPtr<FJsonValue>> Mapping;
        for (const FTrackToSkeletonMap& Track : Data.CompressedTrackToSkeletonMapTable) { Mapping.Add(MakeShared<FJsonValueNumber>(Track.BoneTreeIndex)); }
        Compression->SetArrayField(TEXT("track_to_skeleton"), Mapping);
    }
    if (bRootTrackOnly && bAnimationPose)
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Sequence pose sampling modes are mutually exclusive."));
        return nullptr;
    }
    const FReferenceSkeleton& Skeleton = Sequence->GetSkeleton()->GetReferenceSkeleton();
    USkeletalMesh* PoseMesh = nullptr;
    if (Request->HasField(TEXT("mesh_asset")))
    {
        FString MeshPath;
        if (!bAnimationPose || !Request->TryGetStringField(TEXT("mesh_asset"), MeshPath) || MeshPath.IsEmpty()) { return nullptr; }
        PoseMesh = LoadObject<USkeletalMesh>(nullptr, *MeshPath);
        if (PoseMesh == nullptr || PoseMesh->GetSkeleton() != Sequence->GetSkeleton()) { return nullptr; }
        for (int32 Index = 0; Index < Skeleton.GetNum(); ++Index)
        {
            if (PoseMesh->GetRefSkeleton().FindBoneIndex(Skeleton.GetBoneName(Index)) == INDEX_NONE) { return nullptr; }
        }
    }
    const FReferenceSkeleton& RequiredSkeleton = PoseMesh != nullptr ? PoseMesh->GetRefSkeleton() : Skeleton;
    TArray<FBoneIndexType> BoneIndices;
    FBoneContainer RequiredBones;
    if (bAnimationPose || bCompressedTracks)
    {
        for (int32 Index = 0; Index < RequiredSkeleton.GetNum(); ++Index)
        {
            BoneIndices.Add(Index);
        }
        UObject& RequiredAsset = PoseMesh != nullptr ? static_cast<UObject&>(*PoseMesh) : static_cast<UObject&>(*Sequence->GetSkeleton());
        RequiredBones.InitializeTo(BoneIndices, UE::Anim::FCurveFilterSettings(), RequiredAsset);
        if (bRawAdditivePose) { RequiredBones.SetUseRAWData(true); }
    }
    TArray<TSharedPtr<FJsonValue>> Frames;
    for (const TSharedPtr<FJsonValue>& Value : *Times)
    {
        double Time;
        if (!Value->TryGetNumber(Time) || !FMath::IsFinite(Time) || Time < 0.0 || Time > Sequence->GetPlayLength())
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Sequence pose probe time is outside the asset."));
            return nullptr;
        }
        TSharedRef<FJsonObject> Frame = MakeShared<FJsonObject>();
        Frame->SetNumberField(TEXT("time"), Time);
        FMemMark Mark(FMemStack::Get());
        FCompactPose Pose;
        FBlendedCurve Curve;
        UE::Anim::FStackAttributeContainer Attributes;
        TArray<FTransform> CodecPose;
        if (bCompressedTracks)
        {
            const FAnimExtractContext Context(Time, false);
            if (FSequencePoseAccess::UsesRawData(*Sequence, RequiredBones, Context)) { return nullptr; }
            const auto Scoped = Sequence->GetCompressedData(Context);
            const FCompressedAnimSequence& Data = Scoped.Get();
            CodecPose = Skeleton.GetRefBonePose();
            if (Sequence->IsValidAdditive()) { for (FTransform& Transform : CodecPose) { Transform = FTransform(FQuat::Identity, FVector::ZeroVector, FVector::ZeroVector); } }
            BoneTrackArray Pairs;
            for (int32 TrackIndex = 0; TrackIndex < Data.CompressedTrackToSkeletonMapTable.Num(); ++TrackIndex)
            {
                Pairs.Add(BoneTrackPair(Data.CompressedTrackToSkeletonMapTable[TrackIndex].BoneTreeIndex, TrackIndex));
            }
            FAnimSequenceDecompressionContext DecompContext(Sequence->GetSamplingFrameRate(), Sequence->GetSamplingFrameRate().AsFrameTime(Sequence->GetPlayLength()).RoundToFrame().Value,
                Sequence->Interpolation, Sequence->GetRetargetTransformsSourceName(), *Data.CompressedDataStructure, Sequence->GetSkeleton()->GetRefLocalPoses(), Data.CompressedTrackToSkeletonMapTable, Sequence->GetSkeleton(), Sequence->IsValidAdditive(), Sequence->AdditiveAnimType);
            DecompContext.Seek(Time);
            TArrayView<FTransform> Atoms(CodecPose);
            Data.BoneCompressionCodec->DecompressPose(DecompContext, Pairs, Pairs, Pairs, Atoms);
            if (!Data.IsCurveDataValid(Sequence)) { return nullptr; }
            Curve.InitFrom(RequiredBones);
            Data.CurveCompressionCodec->DecompressCurves(Data, Curve, Time);
            TSharedRef<FJsonObject> CodecCurves = MakeShared<FJsonObject>();
            Curve.ForEachElement([&CodecCurves](const UE::Anim::FCurveElement& Element)
            {
                CodecCurves->SetNumberField(Element.Name.ToString(), Element.Value);
            });
            Frame->SetObjectField(TEXT("codec_curves"), CodecCurves);
        }
        if (bAnimationPose)
        {
            Pose.SetBoneContainer(&RequiredBones);
            Curve.InitFrom(RequiredBones);
            FAnimationPoseData PoseData(Pose, Curve, Attributes);
            const FAnimExtractContext ExtractionContext(Time, false);
            Frame->SetBoolField(TEXT("compressed_data_valid"), Sequence->IsCompressedDataValid());
            Frame->SetStringField(TEXT("pose_evaluation_mode"), bRawAbsolutePose || FSequencePoseAccess::UsesRawData(*Sequence, RequiredBones, ExtractionContext) ? TEXT("raw_model") : TEXT("compressed"));
            if (bCaptureSourceStages && !bRawAbsolutePose && !FSequencePoseAccess::UsesRawData(*Sequence, RequiredBones, ExtractionContext)) { return nullptr; }
            if (bRawAbsolutePose) { Sequence->GetBonePose(PoseData, ExtractionContext, true); }
            else { Sequence->GetAnimationPose(PoseData, ExtractionContext); }
            if (bCaptureSourceStages)
            {
                const TSharedPtr<FJsonObject> Stages = CaptureSequenceRigStages(*Sequence, Time, RequiredBones);
                if (!Stages.IsValid()) { return nullptr; }
                FCSPose<FCompactPose> ComponentPose;
                ComponentPose.InitPose(Pose);
                TArray<TSharedPtr<FJsonValue>> VirtualReplay;
                for (const FVirtualBoneCompactPoseData& Virtual : RequiredBones.GetVirtualBoneCompactPoseData())
                {
                    TSharedRef<FJsonObject> ReplayItem = MakeShared<FJsonObject>();
                    const FSkeletonPoseBoneIndex SkeletonIndex = RequiredBones.GetSkeletonPoseIndexFromCompactPoseIndex(Virtual.VBIndex);
                    ReplayItem->SetStringField(TEXT("name"), Skeleton.GetBoneName(SkeletonIndex.GetInt()).ToString());
                    const FTransform Source = ComponentPose.GetComponentSpaceTransform(Virtual.SourceIndex);
                    const FTransform Target = ComponentPose.GetComponentSpaceTransform(Virtual.TargetIndex);
                    ReplayItem->SetObjectField(TEXT("source_component"), CaptureTransform(Source));
                    ReplayItem->SetObjectField(TEXT("target_component"), CaptureTransform(Target));
                    ReplayItem->SetObjectField(TEXT("relative"), CaptureTransform(Target.GetRelativeTransform(Source)));
                    VirtualReplay.Add(MakeShared<FJsonValueObject>(ReplayItem));
                }
                Stages->SetArrayField(TEXT("post_pose_virtual_replay"), VirtualReplay);
                Frame->SetObjectField(TEXT("source_stages"), Stages.ToSharedRef());
            }
            TSharedRef<FJsonObject> Curves = MakeShared<FJsonObject>();
            Curve.ForEachElement([&Curves](const UE::Anim::FCurveElement& Element)
            {
                Curves->SetNumberField(Element.Name.ToString(), Element.Value);
            });
            Frame->SetObjectField(TEXT("curves"), Curves);
        }
        TArray<TSharedPtr<FJsonValue>> Bones;
        for (int32 Index = 0; Index < (bRootTrackOnly ? 1 : Skeleton.GetNum()); ++Index)
        {
            FTransform Transform = Skeleton.GetRefBonePose()[Index];
            if (bAnimationPose)
            {
                Transform = Pose[RequiredBones.GetCompactPoseIndexFromSkeletonPoseIndex(FSkeletonPoseBoneIndex(Index))];
            }
            else if (bRootTrackOnly)
            {
                Transform = Sequence->ExtractRootTrackTransform(FAnimExtractContext(Time), nullptr);
            }
            else
            {
                const FAnimExtractContext TrackContext(Time, false);
                Sequence->GetBoneTransform(Transform, FSkeletonPoseBoneIndex(Index), TrackContext, !bCompressedTracks);
            }
            if (Transform.ContainsNaN())
            {
                UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Sequence pose probe produced a non-finite transform."));
                return nullptr;
            }
            TSharedRef<FJsonObject> Bone = MakeShared<FJsonObject>();
            Bone->SetStringField(TEXT("name"), Skeleton.GetBoneName(Index).ToString());
            Bone->SetNumberField(TEXT("parent_index"), Skeleton.GetParentIndex(Index));
            Bone->SetObjectField(TEXT("local_transform"), CaptureTransform(Transform));
            if (bCompressedTracks)
            {
                Bone->SetObjectField(TEXT("reference_pose"), CaptureTransform(Skeleton.GetRefBonePose()[Index]));
                FTransform Replayed = Index == 0 ? Transform : CodecPose[Index];
                Bone->SetObjectField(TEXT("codec_pose"), CaptureTransform(Replayed));
                Bone->SetNumberField(TEXT("retarget_mode"), Sequence->GetSkeleton()->GetBoneTranslationRetargetingMode(Index));
                FAnimationRuntime::RetargetBoneTransform(Sequence->GetSkeleton(), Sequence->GetRetargetTransformsSourceName(), Sequence->GetRetargetTransforms(), Replayed, Index,
                    RequiredBones.GetCompactPoseIndexFromSkeletonPoseIndex(FSkeletonPoseBoneIndex(Index)), RequiredBones, Sequence->IsValidAdditive());
                Bone->SetObjectField(TEXT("retargeted_pose"), CaptureTransform(Replayed));
            }
            Bones.Add(MakeShared<FJsonValueObject>(Bone));
        }
        Frame->SetArrayField(TEXT("bones"), Bones);
        Frames.Add(MakeShared<FJsonValueObject>(Frame));
    }
    TSharedRef<FJsonObject> Probe = MakeShared<FJsonObject>();
    Probe->SetStringField(TEXT("sampling_mode"), bRawAdditivePose ? TEXT("raw_additive_pose") : bRawAbsolutePose ? TEXT("raw_absolute_pose") : bAnimationPose ? TEXT("animation_pose") : bRootTrackOnly ? TEXT("root_track") : bCompressedTracks ? TEXT("compressed_local_tracks") : TEXT("raw_local_tracks"));
    if (bRawAdditivePose) { Probe->SetBoolField(TEXT("required_bones_raw"), RequiredBones.ShouldUseRawData()); }
    if (PoseMesh != nullptr) { Probe->SetStringField(TEXT("required_bones_mesh"), ObjectPath(RequiredBones.GetSkeletalMeshAsset())); }
    if (bCompressedTracks) { Probe->SetObjectField(TEXT("compression"), Compression.ToSharedRef()); }
    Probe->SetNumberField(TEXT("duration"), Sequence->GetPlayLength());
    Probe->SetArrayField(TEXT("frames"), Frames);
    if (bCaptureSourceChannels) { Probe->SetObjectField(TEXT("source_channels"), SourceChannels.ToSharedRef()); }
    return Probe;
}

TSharedPtr<FJsonObject> CaptureSequenceClockProbe(UObject* Asset, const FString& RequestPath)
{
    FMemMark Mark(FMemStack::Get());
    UAnimSequenceBase* Sequence = Cast<UAnimSequenceBase>(Asset);
    FString Text;
    TSharedPtr<FJsonObject> Request;
    const TArray<TSharedPtr<FJsonValue>>* Samples = nullptr;
    if (Sequence == nullptr || Sequence->GetPlayLength() <= 0.0f || !FFileHelper::LoadFileToString(Text, *RequestPath) ||
        !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) ||
        !Request->TryGetArrayField(TEXT("samples"), Samples) || Samples->IsEmpty() || Samples->Num() > 512) { return nullptr; }
    UAnimSequenceBase* Follower = nullptr;
    if (Request->HasField(TEXT("follower_asset")))
    {
        FString FollowerPath;
        if (!Request->TryGetStringField(TEXT("follower_asset"), FollowerPath)) { return nullptr; }
        Follower = LoadObject<UAnimSequenceBase>(nullptr, *FollowerPath);
        if (Follower == nullptr || Follower->GetPlayLength() <= 0.0f) { return nullptr; }
    }
    const TArray<FName> NoMarkers;
    TArray<TSharedPtr<FJsonValue>> Rows;
    for (const TSharedPtr<FJsonValue>& SampleValue : *Samples)
    {
        const TSharedPtr<FJsonObject>* Sample = nullptr;
        const TArray<TSharedPtr<FJsonValue>>* Deltas = nullptr;
        FString Name;
        double Start, Rate;
        if (!SampleValue->TryGetObject(Sample) || !(*Sample)->TryGetStringField(TEXT("name"), Name) ||
            !(*Sample)->TryGetNumberField(TEXT("start"), Start) || !FMath::IsFinite(static_cast<float>(Start)) || Start < 0.0 || Start > Sequence->GetPlayLength() ||
            !(*Sample)->TryGetNumberField(TEXT("rate"), Rate) || !FMath::IsFinite(static_cast<float>(Rate)) ||
            !(*Sample)->TryGetArrayField(TEXT("deltas"), Deltas) || Deltas->IsEmpty() || Deltas->Num() > 512) { return nullptr; }
        if (Follower != nullptr && Start > Follower->GetPlayLength()) { return nullptr; }
        bool Looping = false;
        if ((*Sample)->HasField(TEXT("looping")) && !(*Sample)->TryGetBoolField(TEXT("looping"), Looping)) { return nullptr; }
        float Time = static_cast<float>(Start);
        float FollowerTime = Time;
        FDeltaTimeRecord DeltaRecord;
        FMarkerTickRecord MarkerRecord;
        FDeltaTimeRecord FollowerDeltaRecord;
        FMarkerTickRecord FollowerMarkerRecord;
        TArray<TSharedPtr<FJsonValue>> Frames;
        for (const TSharedPtr<FJsonValue>& DeltaValue : *Deltas)
        {
            double Delta;
            if (!DeltaValue->TryGetNumber(Delta) || Delta < 0.0 || !FMath::IsFinite(static_cast<float>(Delta)) ||
                !FMath::IsFinite(static_cast<float>(Delta) * static_cast<float>(Rate) * Sequence->RateScale)) { return nullptr; }
            FAnimTickRecord Tick(Sequence, Looping, static_cast<float>(Rate), false, 1.0f, Time, MarkerRecord);
            Tick.DeltaTimeRecord = &DeltaRecord;
            Tick.bCanUseMarkerSync = false;
            FAnimNotifyQueue Queue;
            FAnimAssetTickContext Context(static_cast<float>(Delta), ERootMotionMode::NoRootMotionExtraction, Follower == nullptr, NoMarkers);
            Sequence->TickAssetPlayer(Tick, Queue, Context);
            TSharedRef<FJsonObject> Frame = MakeShared<FJsonObject>();
            Frame->SetNumberField(TEXT("position"), Time);
            Frame->SetNumberField(TEXT("previous"), DeltaRecord.GetPrevious());
            Frame->SetBoolField(TEXT("has_previous"), DeltaRecord.IsPreviousValid());
            Frame->SetNumberField(TEXT("playback_delta"), DeltaRecord.Delta);
            Frame->SetNumberField(TEXT("ratio"), Context.GetAnimationPositionRatio());
            if (Follower != nullptr)
            {
                FAnimTickRecord FollowerTick(Follower, Looping, static_cast<float>(Rate), false, 1.0f, FollowerTime, FollowerMarkerRecord);
                FollowerTick.DeltaTimeRecord = &FollowerDeltaRecord;
                FollowerTick.bCanUseMarkerSync = false;
                Context.ConvertToFollower();
                Follower->TickAssetPlayer(FollowerTick, Queue, Context);
                TSharedRef<FJsonObject> FollowerFrame = MakeShared<FJsonObject>();
                FollowerFrame->SetNumberField(TEXT("position"), FollowerTime);
                FollowerFrame->SetNumberField(TEXT("previous"), FollowerDeltaRecord.GetPrevious());
                FollowerFrame->SetBoolField(TEXT("has_previous"), FollowerDeltaRecord.IsPreviousValid());
                FollowerFrame->SetNumberField(TEXT("playback_delta"), FollowerDeltaRecord.Delta);
                Frame->SetObjectField(TEXT("follower"), FollowerFrame);
            }
            Frames.Add(MakeShared<FJsonValueObject>(Frame));
        }
        TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
        Row->SetStringField(TEXT("name"), Name);
        if ((*Sample)->HasField(TEXT("looping"))) { Row->SetBoolField(TEXT("looping"), Looping); }
        Row->SetArrayField(TEXT("frames"), Frames);
        Rows.Add(MakeShared<FJsonValueObject>(Row));
        UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Sequence clock sample %d/%d captured."), Rows.Num(), Samples->Num());
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetNumberField(TEXT("duration"), Sequence->GetPlayLength());
    Result->SetNumberField(TEXT("rate_scale"), Sequence->RateScale);
    Result->SetArrayField(TEXT("samples"), Rows);
    if (Follower != nullptr)
    {
        TSharedRef<FJsonObject> FollowerResult = MakeShared<FJsonObject>();
        FollowerResult->SetStringField(TEXT("asset_path"), Follower->GetPathName());
        FollowerResult->SetNumberField(TEXT("duration"), Follower->GetPlayLength());
        float Times[2] = {0.0f, 0.0f};
        FMarkerTickRecord Markers[2];
        TArray<FAnimTickRecord> Candidates;
        Candidates.Add(FAnimTickRecord(Sequence, false, 1.0f, false, 1.0f, Times[0], Markers[0]));
        Candidates.Add(FAnimTickRecord(Follower, false, 1.0f, false, 1.0f, Times[1], Markers[1]));
        for (FAnimTickRecord& Player : Candidates) { Player.LeaderScore = 1.0f; }
        Candidates.Sort();
        TArray<TSharedPtr<FJsonValue>> Order;
        for (const FAnimTickRecord& Player : Candidates)
        {
            Order.Add(MakeShared<FJsonValueNumber>(Player.MarkerTickRecord == &Markers[0] ? 0 : 1));
        }
        FollowerResult->SetArrayField(TEXT("equal_weight_order"), Order);
        const TArray<FName>* LeaderMarkers = Sequence->GetUniqueMarkerNames();
        const TArray<FName>* FollowerMarkers = Follower->GetUniqueMarkerNames();
        FollowerResult->SetNumberField(TEXT("leader_marker_count"), LeaderMarkers == nullptr ? 0 : LeaderMarkers->Num());
        FollowerResult->SetNumberField(TEXT("follower_marker_count"), FollowerMarkers == nullptr ? 0 : FollowerMarkers->Num());
        Result->SetObjectField(TEXT("follower"), FollowerResult);
    }
    return Result;
}

TSharedPtr<FJsonObject> CaptureCompactPose(const FCompactPose& Pose, const FBlendedCurve& Curve)
{
    const FBoneContainer& RequiredBones = Pose.GetBoneContainer();
    TSharedRef<FJsonObject> PoseObject = MakeShared<FJsonObject>();
    TSharedRef<FJsonObject> Curves = MakeShared<FJsonObject>();
    bool bFiniteCurves = true;
    Curve.ForEachElement([&Curves, &bFiniteCurves](const UE::Anim::FCurveElement& Element)
    {
        bFiniteCurves &= FMath::IsFinite(Element.Value);
        Curves->SetNumberField(Element.Name.ToString(), Element.Value);
    });
    if (!bFiniteCurves) { return nullptr; }
    PoseObject->SetObjectField(TEXT("curves"), Curves);
    TArray<TSharedPtr<FJsonValue>> Bones;
    const FReferenceSkeleton& Skeleton = RequiredBones.GetSkeletonAsset()->GetReferenceSkeleton();
    for (int32 BoneIndex = 0; BoneIndex < Skeleton.GetNum(); ++BoneIndex)
    {
        const FCompactPoseBoneIndex Compact = RequiredBones.GetCompactPoseIndexFromSkeletonPoseIndex(FSkeletonPoseBoneIndex(BoneIndex));
        if (Compact.GetInt() == INDEX_NONE) { return nullptr; }
        const FTransform Transform = Pose[Compact];
        if (Transform.ContainsNaN()) { return nullptr; }
        TSharedRef<FJsonObject> Bone = MakeShared<FJsonObject>();
        Bone->SetStringField(TEXT("name"), Skeleton.GetBoneName(BoneIndex).ToString());
        Bone->SetNumberField(TEXT("parent_index"), Skeleton.GetParentIndex(BoneIndex));
        Bone->SetObjectField(TEXT("local_transform"), CaptureTransform(Transform));
        Bones.Add(MakeShared<FJsonValueObject>(Bone));
    }
    PoseObject->SetArrayField(TEXT("bones"), Bones);
    return PoseObject;
}

TSharedPtr<FJsonObject> CaptureSyncGroupClockProbe(UObject* Asset, const FString& RequestPath)
{
    FMemMark Mark(FMemStack::Get());
    FString Text;
    TSharedPtr<FJsonObject> Request;
    const TArray<TSharedPtr<FJsonValue>>* Players = nullptr;
    const TArray<TSharedPtr<FJsonValue>>* Samples = nullptr;
    if (!FFileHelper::LoadFileToString(Text, *RequestPath) ||
        !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) ||
        !Request->TryGetArrayField(TEXT("players"), Players) || Players->IsEmpty() || Players->Num() > 16 ||
        !Request->TryGetArrayField(TEXT("samples"), Samples) || Samples->IsEmpty() || Samples->Num() > 512) { return nullptr; }
    bool bCapturePose = false;
    if (Request->HasField(TEXT("capture_pose")) &&
        (Request->TryGetField(TEXT("capture_pose"))->Type != EJson::Boolean || !Request->TryGetBoolField(TEXT("capture_pose"), bCapturePose))) { return nullptr; }
    bool bPrepareCompressed = false;
    if (Request->HasField(TEXT("prepare_compressed")) &&
        (Request->TryGetField(TEXT("prepare_compressed"))->Type != EJson::Boolean || !Request->TryGetBoolField(TEXT("prepare_compressed"), bPrepareCompressed))) { return nullptr; }
    if (bPrepareCompressed && !bCapturePose) { return nullptr; }
    bool bCaptureSamplePoses = false;
    if (Request->HasField(TEXT("capture_sample_poses")) &&
        (Request->TryGetField(TEXT("capture_sample_poses"))->Type != EJson::Boolean || !Request->TryGetBoolField(TEXT("capture_sample_poses"), bCaptureSamplePoses))) { return nullptr; }
    if (bCaptureSamplePoses && (!bCapturePose || !bPrepareCompressed)) { return nullptr; }
    TArray<FBoneContainer> RequiredBones;
    if (bCapturePose) { RequiredBones.SetNum(Players->Num()); }
    TArray<UAnimationAsset*> Assets;
    TArray<float> Starts, Rates, AssetRates;
    TArray<FVector> BlendPositions;
    TArray<bool> Loops;
    TArray<TSharedPtr<FJsonValue>> PlayerInfo;
    for (const TSharedPtr<FJsonValue>& Value : *Players)
    {
        const TSharedPtr<FJsonObject>* Player = nullptr;
        FString Path;
        double Start, Rate;
        if (!Value->TryGetObject(Player) || !(*Player)->TryGetStringField(TEXT("asset"), Path) ||
            !(*Player)->TryGetNumberField(TEXT("start"), Start) || !FMath::IsFinite(static_cast<float>(Start)) || Start < 0.0 ||
            !(*Player)->TryGetNumberField(TEXT("rate"), Rate) || !FMath::IsFinite(static_cast<float>(Rate))) { return nullptr; }
        UAnimationAsset* AnimationAsset = LoadObject<UAnimationAsset>(nullptr, *Path);
        UAnimSequenceBase* Sequence = Cast<UAnimSequenceBase>(AnimationAsset);
        UBlendSpace* BlendSpace = Cast<UBlendSpace>(AnimationAsset);
        if (Sequence == nullptr && BlendSpace == nullptr) { return nullptr; }
        if (bCapturePose && Cast<UAnimSequence>(AnimationAsset) == nullptr && BlendSpace == nullptr) { return nullptr; }
        if (Sequence != nullptr && (Sequence->GetPlayLength() <= 0.0f || Start > Sequence->GetPlayLength() ||
            !FMath::IsFinite(Sequence->RateScale))) { return nullptr; }
        FVector BlendPosition = FVector::ZeroVector;
        if (BlendSpace != nullptr)
        {
            const TArray<TSharedPtr<FJsonValue>>* Position = nullptr;
            double X, Y;
            if (Start > 1.0 || !(*Player)->TryGetArrayField(TEXT("blend_position"), Position) || Position->Num() != 2 ||
                !(*Position)[0]->TryGetNumber(X) || !(*Position)[1]->TryGetNumber(Y) ||
                !FMath::IsFinite(static_cast<float>(X)) || !FMath::IsFinite(static_cast<float>(Y))) { return nullptr; }
            BlendPosition = FVector(X, Y, 0.0);
        }
        bool Looped = false;
        if ((*Player)->HasField(TEXT("looping")) && !(*Player)->TryGetBoolField(TEXT("looping"), Looped)) { return nullptr; }
        Assets.Add(AnimationAsset);
        Starts.Add(static_cast<float>(Start));
        Rates.Add(static_cast<float>(Rate));
        AssetRates.Add(Sequence == nullptr ? 1.0f : Sequence->RateScale);
        BlendPositions.Add(BlendPosition);
        Loops.Add(Looped);
        TSharedRef<FJsonObject> Info = MakeShared<FJsonObject>();
        Info->SetStringField(TEXT("asset_path"), AnimationAsset->GetPathName());
        if (Sequence != nullptr)
        {
            Info->SetNumberField(TEXT("duration"), Sequence->GetPlayLength());
            Info->SetNumberField(TEXT("rate_scale"), Sequence->RateScale);
        }
        else
        {
            Info->SetStringField(TEXT("time_unit"), TEXT("normalized"));
            Info->SetObjectField(TEXT("blend_position"), CaptureVector(BlendPosition));
        }
        if (const UAnimSequence* Animation = Cast<UAnimSequence>(Sequence); Animation != nullptr && !Animation->AuthoredSyncMarkers.IsEmpty())
        {
            TArray<TSharedPtr<FJsonValue>> MarkerValues;
            for (const FAnimSyncMarker& Marker : Animation->AuthoredSyncMarkers)
            {
                TSharedRef<FJsonObject> MarkerValue = MakeShared<FJsonObject>();
                MarkerValue->SetStringField(TEXT("name"), Marker.MarkerName.ToString());
                MarkerValue->SetNumberField(TEXT("time"), Marker.Time);
                MarkerValues.Add(MakeShared<FJsonValueObject>(MarkerValue));
            }
            Info->SetArrayField(TEXT("markers"), MarkerValues);
        }
        if (bCapturePose)
        {
            if (bPrepareCompressed)
            {
                if (UAnimSequence* Animation = Cast<UAnimSequence>(AnimationAsset)) { Animation->CacheDerivedDataForCurrentPlatform(); }
                if (BlendSpace != nullptr)
                {
                    for (const FBlendSample& Sample : BlendSpace->GetBlendSamples())
                    {
                        if (Sample.Animation != nullptr) { Sample.Animation->CacheDerivedDataForCurrentPlatform(); }
                    }
                }
            }
            USkeleton* Skeleton = AnimationAsset->GetSkeleton();
            if (Skeleton == nullptr || Skeleton->GetReferenceSkeleton().GetNum() == 0) { return nullptr; }
            TArray<FBoneIndexType> BoneIndices;
            for (int32 Index = 0; Index < Skeleton->GetReferenceSkeleton().GetNum(); ++Index) { BoneIndices.Add(Index); }
            RequiredBones[Assets.Num() - 1].InitializeTo(BoneIndices, UE::Anim::FCurveFilterSettings(), *Skeleton);
            Info->SetStringField(TEXT("pose_skeleton"), ObjectPath(Skeleton));
            Info->SetNumberField(TEXT("pose_bone_count"), BoneIndices.Num());
        }
        PlayerInfo.Add(MakeShared<FJsonValueObject>(Info));
    }
    if (Assets[0] != Asset) { return nullptr; }
    const FName GroupName(TEXT("Probe"));
    TArray<TSharedPtr<FJsonValue>> Rows;
    for (const TSharedPtr<FJsonValue>& Value : *Samples)
    {
        const TSharedPtr<FJsonObject>* Sample = nullptr;
        const TArray<TSharedPtr<FJsonValue>>* Steps = nullptr;
        FString Name;
        if (!Value->TryGetObject(Sample) || !(*Sample)->TryGetStringField(TEXT("name"), Name) ||
            !(*Sample)->TryGetArrayField(TEXT("steps"), Steps) || Steps->IsEmpty() || Steps->Num() > 512) { return nullptr; }
        TArray<float> Times = Starts;
        TArray<FDeltaTimeRecord> Deltas;
        TArray<FMarkerTickRecord> Markers;
        TArray<FBlendFilter> Filters;
        TArray<TArray<FBlendSampleData>> BlendSamples;
        TArray<int32> TriangulationIndices;
        Deltas.SetNum(Assets.Num());
        Markers.SetNum(Assets.Num());
        Filters.SetNum(Assets.Num());
        BlendSamples.SetNum(Assets.Num());
        TriangulationIndices.Init(INDEX_NONE, Assets.Num());
        for (int32 Index = 0; Index < Assets.Num(); ++Index)
        {
            if (UBlendSpace* BlendSpace = Cast<UBlendSpace>(Assets[Index])) { BlendSpace->InitializeFilter(&Filters[Index]); }
        }
        USkeletalMeshComponent* Mesh = NewObject<USkeletalMeshComponent>(GetTransientPackage());
        UAnimInstance* Instance = NewObject<UAnimInstance>(Mesh);
        FAnimInstanceProxy Proxy(Instance);
        UE::Anim::FAnimSync Sync;
        TArray<TSharedPtr<FJsonValue>> Frames;
        for (const TSharedPtr<FJsonValue>& StepValue : *Steps)
        {
            const TSharedPtr<FJsonObject>* Step = nullptr;
            const TArray<TSharedPtr<FJsonValue>>* Weights = nullptr;
            double Delta;
            if (!StepValue->TryGetObject(Step) || !(*Step)->TryGetNumberField(TEXT("delta_seconds"), Delta) ||
                !FMath::IsFinite(static_cast<float>(Delta)) || Delta < 0.0 ||
                !(*Step)->TryGetArrayField(TEXT("weights"), Weights) || Weights->Num() != Assets.Num()) { return nullptr; }
            bool Inertial = false;
            if ((*Step)->HasField(TEXT("inertial")) && !(*Step)->TryGetBoolField(TEXT("inertial"), Inertial)) { return nullptr; }
            TArray<float> StepRates = Rates;
            if ((*Step)->HasField(TEXT("rates")))
            {
                const TArray<TSharedPtr<FJsonValue>>* Values = nullptr;
                if (!(*Step)->TryGetArrayField(TEXT("rates"), Values) || Values->Num() != Assets.Num()) { return nullptr; }
                for (int32 Index = 0; Index < Assets.Num(); ++Index)
                {
                    double Rate;
                    if (!(*Values)[Index]->TryGetNumber(Rate) || !FMath::IsFinite(static_cast<float>(Rate))) { return nullptr; }
                    StepRates[Index] = static_cast<float>(Rate);
                }
            }
            TArray<FVector> Positions = BlendPositions;
            if ((*Step)->HasField(TEXT("blend_positions")))
            {
                const TArray<TSharedPtr<FJsonValue>>* Values = nullptr;
                if (!(*Step)->TryGetArrayField(TEXT("blend_positions"), Values) || Values->Num() != Assets.Num()) { return nullptr; }
                for (int32 Index = 0; Index < Assets.Num(); ++Index)
                {
                    if ((*Values)[Index]->Type == EJson::Null) { continue; }
                    const TArray<TSharedPtr<FJsonValue>>* Position = nullptr;
                    double X, Y;
                    if (Cast<UBlendSpace>(Assets[Index]) == nullptr || !(*Values)[Index]->TryGetArray(Position) || Position->Num() != 2 ||
                        !(*Position)[0]->TryGetNumber(X) || !(*Position)[1]->TryGetNumber(Y) ||
                        !FMath::IsFinite(static_cast<float>(X)) || !FMath::IsFinite(static_cast<float>(Y))) { return nullptr; }
                    Positions[Index] = FVector(X, Y, 0.0);
                }
            }
            if ((*Step)->HasField(TEXT("reset")))
            {
                const TArray<TSharedPtr<FJsonValue>>* Reset = nullptr;
                if (!(*Step)->TryGetArrayField(TEXT("reset"), Reset) || Reset->Num() > Assets.Num()) { return nullptr; }
                for (const TSharedPtr<FJsonValue>& IndexValue : *Reset)
                {
                    double Index;
                    if (!IndexValue->TryGetNumber(Index) || !FMath::IsFinite(Index) || Index < 0.0 || Index >= Assets.Num() || Index != FMath::FloorToDouble(Index) ||
                        Cast<UBlendSpace>(Assets[static_cast<int32>(Index)]) != nullptr) { return nullptr; }
                    Times[static_cast<int32>(Index)] = Starts[static_cast<int32>(Index)];
                    Markers[static_cast<int32>(Index)].Reset();
                }
            }
            Sync.Reset();
            for (int32 Index = 0; Index < Assets.Num(); ++Index)
            {
                if ((*Weights)[Index]->Type == EJson::Null) { continue; }
                double Weight;
                if (!(*Weights)[Index]->TryGetNumber(Weight) || !FMath::IsFinite(static_cast<float>(Weight)) || Weight < 0.0 || Weight > 1.0 ||
                    !FMath::IsFinite(static_cast<float>(Delta) * StepRates[Index] * AssetRates[Index])) { return nullptr; }
                FAnimTickRecord Tick;
                if (UBlendSpace* BlendSpace = Cast<UBlendSpace>(Assets[Index]))
                {
                    Tick = FAnimTickRecord(BlendSpace, Positions[Index], BlendSamples[Index], Filters[Index], Loops[Index], StepRates[Index],
                        false, false, static_cast<float>(Weight), Times[Index], Markers[Index]);
                    Tick.BlendSpace.TriangulationIndex = TriangulationIndices[Index];
                }
                else
                {
                    Tick = FAnimTickRecord(CastChecked<UAnimSequenceBase>(Assets[Index]), Loops[Index], StepRates[Index], false,
                        static_cast<float>(Weight), Times[Index], Markers[Index]);
                }
                Tick.DeltaTimeRecord = &Deltas[Index];
                Tick.bRequestedInertialization = Inertial;
                Sync.AddTickRecord(Tick, UE::Anim::FAnimSyncParams(GroupName, EAnimGroupRole::CanBeLeader, EAnimSyncMethod::SyncGroup));
            }
            Sync.TickAssetPlayerInstances(Proxy, static_cast<float>(Delta));
            const FAnimGroupInstance* Group = Sync.GetSyncGroupMapRead().Find(GroupName);
            TSharedRef<FJsonObject> Frame = MakeShared<FJsonObject>();
            TArray<TSharedPtr<FJsonValue>> Order, Clocks;
            Frame->SetField(TEXT("leader"), MakeShared<FJsonValueNull>());
            if (Group != nullptr)
            {
                for (const FAnimTickRecord& Tick : Group->ActivePlayers)
                {
                    const int32 Index = static_cast<int32>(Tick.TimeAccumulator - Times.GetData());
                    Order.Add(MakeShared<FJsonValueNumber>(Index));
                    if (Cast<UBlendSpace>(Assets[Index]) != nullptr) { TriangulationIndices[Index] = Tick.BlendSpace.TriangulationIndex; }
                }
                if (Group->ActivePlayers.IsValidIndex(Group->GroupLeaderIndex))
                {
                    Frame->SetNumberField(TEXT("leader"), Group->ActivePlayers[Group->GroupLeaderIndex].TimeAccumulator - Times.GetData());
                }
                if (!Group->ValidMarkers.IsEmpty())
                {
                    for (const bool Start : {true, false})
                    {
                        const FMarkerSyncAnimPosition& Position = Start ? Group->MarkerTickContext.GetMarkerSyncStartPosition() : Group->MarkerTickContext.GetMarkerSyncEndPosition();
                        TSharedRef<FJsonObject> MarkerPosition = MakeShared<FJsonObject>();
                        MarkerPosition->SetStringField(TEXT("previous"), Position.PreviousMarkerName.ToString());
                        MarkerPosition->SetStringField(TEXT("next"), Position.NextMarkerName.ToString());
                        MarkerPosition->SetNumberField(TEXT("alpha"), Position.PositionBetweenMarkers);
                        MarkerPosition->SetBoolField(TEXT("valid"), Position.IsValid());
                        Frame->SetObjectField(Start ? TEXT("marker_start") : TEXT("marker_end"), MarkerPosition);
                    }
                }
            }
            Frame->SetArrayField(TEXT("order"), Order);
            Frame->SetNumberField(TEXT("ratio"), Group == nullptr ? 0.0f : Group->AnimLengthRatio);
            Frame->SetNumberField(TEXT("previous_ratio"), Group == nullptr ? 0.0f : Group->PreviousAnimLengthRatio);
            for (int32 Index = 0; Index < Assets.Num(); ++Index)
            {
                FMemMark ClockMark(FMemStack::Get());
                TArray<FCompactPose> CapturedChildPoses;
                TArray<float> CapturedChildWeights;
                TSharedRef<FJsonObject> Clock = MakeShared<FJsonObject>();
                Clock->SetNumberField(TEXT("position"), Times[Index]);
                Clock->SetNumberField(TEXT("previous"), Deltas[Index].GetPrevious());
                Clock->SetNumberField(TEXT("playback_delta"), Deltas[Index].Delta);
                Clock->SetBoolField(TEXT("has_previous"), Deltas[Index].IsPreviousValid());
                Clock->SetBoolField(TEXT("ticked"), (*Weights)[Index]->Type != EJson::Null);
                if (Cast<UBlendSpace>(Assets[Index]) != nullptr)
                {
                    Clock->SetObjectField(TEXT("filtered_position"), CaptureVector(Filters[Index].GetFilterLastOutput()));
                    TArray<TSharedPtr<FJsonValue>> SampleValues;
                    for (const FBlendSampleData& BlendSample : BlendSamples[Index])
                    {
                        TSharedRef<FJsonObject> SampleValue = MakeShared<FJsonObject>();
                        SampleValue->SetNumberField(TEXT("sample_index"), BlendSample.SampleDataIndex);
                        SampleValue->SetStringField(TEXT("animation"), ObjectPath(BlendSample.Animation));
                        SampleValue->SetNumberField(TEXT("weight"), BlendSample.GetClampedWeight());
                        SampleValue->SetNumberField(TEXT("previous_time"), BlendSample.PreviousTime);
                        SampleValue->SetNumberField(TEXT("time"), BlendSample.Time);
                        SampleValue->SetNumberField(TEXT("play_rate"), BlendSample.SamplePlayRate);
                        if (bCapturePose && BlendSample.Animation != nullptr)
                        {
                            const FAnimExtractContext Context(BlendSample.Time, false, BlendSample.DeltaTimeRecord, Loops[Index]);
                            SampleValue->SetStringField(TEXT("pose_evaluation_mode"), FSequencePoseAccess::UsesRawData(*BlendSample.Animation, RequiredBones[Index], Context) ? TEXT("raw_model") : TEXT("compressed"));
                            if (bCaptureSamplePoses && (*Weights)[Index]->Type != EJson::Null)
                            {
                                FCompactPose ChildPose;
                                FBlendedCurve ChildCurve;
                                UE::Anim::FStackAttributeContainer ChildAttributes;
                                ChildPose.SetBoneContainer(&RequiredBones[Index]);
                                ChildCurve.InitFrom(RequiredBones[Index]);
                                FAnimationPoseData ChildData(ChildPose, ChildCurve, ChildAttributes);
                                const float ChildTime = FMath::Clamp(BlendSample.Time, 0.f, BlendSample.Animation->GetPlayLength());
                                const FAnimExtractContext ChildContext(ChildTime, false, BlendSample.DeltaTimeRecord, Loops[Index]);
                                if (FSequencePoseAccess::UsesRawData(*BlendSample.Animation, RequiredBones[Index], ChildContext)) { return nullptr; }
                                BlendSample.Animation->GetAnimationPose(ChildData, ChildContext);
                                const TSharedPtr<FJsonObject> ChildObject = CaptureCompactPose(ChildPose, ChildCurve);
                                if (!ChildObject.IsValid()) { return nullptr; }
                                SampleValue->SetObjectField(TEXT("pose"), ChildObject.ToSharedRef());
                                CapturedChildPoses.Add(MoveTemp(ChildPose));
                                CapturedChildWeights.Add(BlendSample.GetClampedWeight());
                            }
                        }
                        SampleValues.Add(MakeShared<FJsonValueObject>(SampleValue));
                    }
                    Clock->SetArrayField(TEXT("samples"), SampleValues);
                }
                if (bCapturePose)
                {
                    Clock->SetField(TEXT("pose"), MakeShared<FJsonValueNull>());
                    if ((*Weights)[Index]->Type != EJson::Null)
                    {
                        FMemMark PoseMark(FMemStack::Get());
                        FCompactPose Pose;
                        FBlendedCurve Curve;
                        UE::Anim::FStackAttributeContainer Attributes;
                        Pose.SetBoneContainer(&RequiredBones[Index]);
                        Curve.InitFrom(RequiredBones[Index]);
                        FAnimationPoseData PoseData(Pose, Curve, Attributes);
                        const FAnimExtractContext ExtractionContext(Times[Index], false, Deltas[Index], Loops[Index]);
                        if (const UBlendSpace* BlendSpace = Cast<UBlendSpace>(Assets[Index]))
                        {
                            BlendSpace->GetAnimationPose(BlendSamples[Index], ExtractionContext, PoseData);
                            if (bCaptureSamplePoses)
                            {
                                if (CapturedChildPoses.Num() != BlendSamples[Index].Num()) { return nullptr; }
                                FCompactPose InnerPose;
                                FBlendedCurve InnerCurve;
                                UE::Anim::FStackAttributeContainer InnerAttributes;
                                InnerPose.SetBoneContainer(&RequiredBones[Index]);
                                InnerCurve.InitFrom(RequiredBones[Index]);
                                FAnimationPoseData InnerData(InnerPose, InnerCurve, InnerAttributes);
                                FAnimationRuntime::BlendPosesTogether(CapturedChildPoses, {}, {}, CapturedChildWeights, InnerData);
                                const TSharedPtr<FJsonObject> InnerObject = CaptureCompactPose(InnerPose, InnerCurve);
                                if (!InnerObject.IsValid()) { return nullptr; }
                                Clock->SetObjectField(TEXT("blend_inner_pose"), InnerObject.ToSharedRef());
                            }
                        }
                        else
                        {
                            Clock->SetStringField(TEXT("pose_evaluation_mode"), FSequencePoseAccess::UsesRawData(*CastChecked<UAnimSequence>(Assets[Index]), RequiredBones[Index], ExtractionContext) ? TEXT("raw_model") : TEXT("compressed"));
                            CastChecked<UAnimSequenceBase>(Assets[Index])->GetAnimationPose(PoseData, ExtractionContext);
                        }
                        const TSharedPtr<FJsonObject> PoseObject = CaptureCompactPose(Pose, Curve);
                        if (!PoseObject.IsValid()) { return nullptr; }
                        Clock->SetObjectField(TEXT("pose"), PoseObject.ToSharedRef());
                    }
                }
                Clocks.Add(MakeShared<FJsonValueObject>(Clock));
            }
            Frame->SetArrayField(TEXT("players"), Clocks);
            Frames.Add(MakeShared<FJsonValueObject>(Frame));
        }
        TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
        Row->SetStringField(TEXT("name"), Name);
        Row->SetArrayField(TEXT("frames"), Frames);
        Rows.Add(MakeShared<FJsonValueObject>(Row));
        UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Sync group clock sample %d/%d captured."), Rows.Num(), Samples->Num());
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetArrayField(TEXT("players"), PlayerInfo);
    Result->SetArrayField(TEXT("samples"), Rows);
    return Result;
}

struct FSourcePoseProbeLeaf : FAnimNode_Base
{
    TArray<FTransform> Transforms;
    TMap<FName, float> Curves;
    float UpdateWeight = 0.f;
    float RootMotionWeight = 0.f;
    virtual void Update_AnyThread(const FAnimationUpdateContext& Context) override
    {
        UpdateWeight = Context.GetFinalBlendWeight();
        RootMotionWeight = Context.GetRootMotionWeightModifier();
    }
    virtual void Evaluate_AnyThread(FPoseContext& Output) override
    {
        Output.Pose.ResetToRefPose();
        for (int32 Index = 0; Index < Transforms.Num(); ++Index) { Output.Pose[FCompactPoseBoneIndex(Index)] = Transforms[Index]; }
        Output.Curve.Empty();
        for (const auto& Curve : Curves) { Output.Curve.Set(Curve.Key, Curve.Value); }
    }
};

bool ReadSourcePoseProbeLeaf(const FReferenceSkeleton& Reference, const TSharedPtr<FJsonValue>& Value, FSourcePoseProbeLeaf& Leaf)
{
    const TSharedPtr<FJsonObject>* Object = nullptr;
    const TSharedPtr<FJsonObject>* Curves = nullptr;
    const TArray<TSharedPtr<FJsonValue>>* Transforms = nullptr;
    if (!Value->TryGetObject(Object) || !(*Object)->TryGetArrayField(TEXT("bone_transforms"), Transforms) ||
        Transforms->Num() != Reference.GetNum() || !(*Object)->TryGetObjectField(TEXT("curves"), Curves)) { return false; }
    for (const TSharedPtr<FJsonValue>& TransformValue : *Transforms)
    {
        const TSharedPtr<FJsonObject>* Transform = nullptr;
        if (!TransformValue->TryGetObject(Transform)) { return false; }
        double Components[10];
        int32 Offset = 0;
        for (const TCHAR* Field : {TEXT("translation_cm"), TEXT("rotation"), TEXT("scale")})
        {
            const TArray<TSharedPtr<FJsonValue>>* Values = nullptr;
            const int32 Count = Offset == 3 ? 4 : 3;
            if (!(*Transform)->TryGetArrayField(Field, Values) || Values->Num() != Count) { return false; }
            for (const TSharedPtr<FJsonValue>& Component : *Values)
            {
                if (Component->Type != EJson::Number || !Component->TryGetNumber(Components[Offset]) || !FMath::IsFinite(Components[Offset])) { return false; }
                ++Offset;
            }
        }
        const FQuat Rotation(Components[3], Components[4], Components[5], Components[6]);
        if (!Rotation.IsNormalized()) { return false; }
        Leaf.Transforms.Add(FTransform(Rotation, FVector(Components[0], Components[1], Components[2]), FVector(Components[7], Components[8], Components[9])));
    }
    for (const auto& Curve : (*Curves)->Values)
    {
        double Amount;
        if (Curve.Value->Type != EJson::Number || !Curve.Value->TryGetNumber(Amount) || !FMath::IsFinite(static_cast<float>(Amount))) { return false; }
        Leaf.Curves.Add(FName(*Curve.Key), static_cast<float>(Amount));
    }
    return true;
}

TSharedPtr<FJsonObject> CaptureBlendListProbe(UObject* Asset, const FString& RequestPath)
{
    FMemMark Mark(FMemStack::Get());
    FString Text, ModeName;
    TSharedPtr<FJsonObject> Request;
    const TArray<TSharedPtr<FJsonValue>>* Times = nullptr;
    const TArray<TSharedPtr<FJsonValue>>* Samples = nullptr;
    if (!FFileHelper::LoadFileToString(Text, *RequestPath) || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) ||
        !Request->TryGetStringField(TEXT("mode"), ModeName) || !Request->TryGetArrayField(TEXT("blend_times"), Times) ||
        Times->IsEmpty() || Times->Num() > 8 || !Request->TryGetArrayField(TEXT("samples"), Samples) || Samples->IsEmpty() || Samples->Num() > 512) { return nullptr; }
    const int64 Mode = StaticEnum<EAlphaBlendOption>()->GetValueByNameString(ModeName);
    if (Mode == INDEX_NONE) { return nullptr; }
    const FAnimNode_TwoWayBlend* TwoWayTemplate = nullptr;
    int32 TwoWayChild = INDEX_NONE;
    if (Request->HasField(TEXT("two_way")))
    {
        const TSharedPtr<FJsonObject>* Fields = nullptr;
        FString PropertyName;
        double Child;
        UBlueprint* Blueprint = Cast<UBlueprint>(Asset);
        UClass* Class = Blueprint != nullptr ? Blueprint->GeneratedClass : nullptr;
        if (Class == nullptr || !Request->TryGetObjectField(TEXT("two_way"), Fields) ||
            !(*Fields)->TryGetStringField(TEXT("property"), PropertyName) || !(*Fields)->TryGetNumberField(TEXT("child"), Child) ||
            !FMath::IsFinite(Child) || Child < 0 || Child >= Times->Num() || Child != FMath::FloorToDouble(Child)) { return nullptr; }
        const FStructProperty* Property = FindFProperty<FStructProperty>(Class, *PropertyName);
        if (Property == nullptr || Property->Struct != FAnimNode_TwoWayBlend::StaticStruct()) { return nullptr; }
        TwoWayTemplate = Property->ContainerPtrToValuePtr<FAnimNode_TwoWayBlend>(Class->GetDefaultObject());
        if (TwoWayTemplate->AlphaInputType != EAnimAlphaInputType::Float) { return nullptr; }
        TwoWayChild = static_cast<int32>(Child);
    }
    int32 CachedChild = INDEX_NONE;
    if (Request->HasField(TEXT("cached_child")))
    {
        double Child;
        if (!Request->TryGetNumberField(TEXT("cached_child"), Child) || !FMath::IsFinite(Child) ||
            Child < 0 || Child >= Times->Num() || Child != FMath::FloorToDouble(Child) || Child == TwoWayChild) { return nullptr; }
        CachedChild = static_cast<int32>(Child);
    }
    const FAnimNode_TwoWayBlend* OuterTemplate = nullptr;
    if (Request->HasField(TEXT("outer_two_way")))
    {
        FString PropertyName;
        UBlueprint* Blueprint = Cast<UBlueprint>(Asset);
        UClass* Class = Blueprint != nullptr ? Blueprint->GeneratedClass : nullptr;
        if (CachedChild == INDEX_NONE || Class == nullptr || !Request->TryGetStringField(TEXT("outer_two_way"), PropertyName)) { return nullptr; }
        const FStructProperty* Property = FindFProperty<FStructProperty>(Class, *PropertyName);
        if (Property == nullptr || Property->Struct != FAnimNode_TwoWayBlend::StaticStruct()) { return nullptr; }
        OuterTemplate = Property->ContainerPtrToValuePtr<FAnimNode_TwoWayBlend>(Class->GetDefaultObject());
        if (OuterTemplate->AlphaInputType != EAnimAlphaInputType::Float) { return nullptr; }
    }
    struct FProbeLeaf : FAnimNode_Base
    {
        FSourcePoseProbeLeaf Pose;
        int32 Initializations = 0;
        TArray<float> Weights;
        FAnimNode_Base* Forward = nullptr;
        TArray<int32>* Order = nullptr;
        int32 Index = INDEX_NONE;
        virtual void Evaluate_AnyThread(FPoseContext& Output) override { Pose.Evaluate_AnyThread(Output); }
        virtual void Initialize_AnyThread(const FAnimationInitializeContext& Context) override { ++Initializations; if (Forward) { Forward->Initialize_AnyThread(Context); } }
        virtual void Update_AnyThread(const FAnimationUpdateContext& Context) override { Weights.Add(Context.GetFinalBlendWeight()); if (Order) { Order->Add(Index); } if (Forward) { Forward->Update_AnyThread(Context); } }
    };
    struct FProbeTwoWay : FAnimNode_TwoWayBlend
    {
        void Configure(const FAnimNode_TwoWayBlend& Template)
        {
            AlphaInputType = Template.AlphaInputType;
            AlphaScaleBias = Template.AlphaScaleBias;
            AlphaScaleBiasClamp = Template.AlphaScaleBiasClamp;
            bResetChildOnActivation = FindFProperty<FBoolProperty>(StaticStruct(), TEXT("bResetChildOnActivation"))->GetPropertyValue_InContainer(&Template);
            bAlwaysUpdateChildren = FindFProperty<FBoolProperty>(StaticStruct(), TEXT("bAlwaysUpdateChildren"))->GetPropertyValue_InContainer(&Template);
        }
        float Value() const { return InternalBlendAlpha; }
    };
    struct FProbeBlend : FAnimNode_BlendListBase
    {
        int32 Active = 0;
        virtual int32 GetActiveChildIndex() override { return Active; }
        void Link(TArray<FProbeLeaf>& Leaves)
        {
            BlendPose.SetNum(Leaves.Num());
            for (int32 Index = 0; Index < Leaves.Num(); ++Index) { BlendPose[Index].SetLinkNode(&Leaves[Index]); }
        }
        float Weight(int32 Index) const { return PerBlendData[Index].Weight; }
        float Duration() const { return PerBlendData[Active].Blend.GetBlendTime(); }
    };
    struct FProbeCache : FAnimNode_SaveCachedPose
    {
        TArray<float> Requests;
        virtual void Update_AnyThread(const FAnimationUpdateContext& Context) override
        {
            Requests.Add(Context.GetFinalBlendWeight());
            FAnimNode_SaveCachedPose::Update_AnyThread(Context);
        }
    };
    struct FProbeProxy : FAnimInstanceProxy
    {
        using FAnimInstanceProxy::Initialize;
        explicit FProbeProxy(UAnimInstance* Instance) : FAnimInstanceProxy(Instance) {}
        void InitializeGraph() { InitializationCounter.Increment(); }
    };
    TArray<TSharedPtr<FJsonValue>> Rows;
    for (const TSharedPtr<FJsonValue>& SampleValue : *Samples)
    {
        const TSharedPtr<FJsonObject>* Sample = nullptr;
        const TArray<TSharedPtr<FJsonValue>>* Steps = nullptr;
        FString Name;
        if (!SampleValue->TryGetObject(Sample) || !(*Sample)->TryGetStringField(TEXT("name"), Name) ||
            !(*Sample)->TryGetArrayField(TEXT("steps"), Steps) || Steps->IsEmpty() || Steps->Num() > 512) { return nullptr; }
        USkeletalMeshComponent* Mesh = NewObject<USkeletalMeshComponent>(GetTransientPackage());
        const bool bCapturePose = (*Sample)->HasField(TEXT("poses"));
        const TArray<TSharedPtr<FJsonValue>>* Poses = nullptr;
        UBlueprint* Blueprint = Cast<UBlueprint>(Asset);
        UAnimBlueprintGeneratedClass* Class = Blueprint != nullptr ? Cast<UAnimBlueprintGeneratedClass>(Blueprint->GeneratedClass) : nullptr;
        const IAnimClassInterface* Interface = Class != nullptr ? IAnimClassInterface::GetFromClass(Class) : nullptr;
        USkeleton* Skeleton = Interface != nullptr ? Interface->GetTargetSkeleton() : nullptr;
        if (bCapturePose && (Skeleton == nullptr || TwoWayTemplate != nullptr || OuterTemplate != nullptr || CachedChild != INDEX_NONE ||
            !(*Sample)->TryGetArrayField(TEXT("poses"), Poses) || Poses->Num() != Times->Num())) { return nullptr; }
        UAnimInstance* Instance = bCapturePose ? NewObject<UAnimInstance>(Mesh, Class) : NewObject<UAnimInstance>(Mesh);
        FProbeProxy Proxy(Instance);
        if (bCapturePose)
        {
            Proxy.Initialize(Instance);
            TArray<FBoneIndexType> BoneIndices;
            for (int32 Index = 0; Index < Skeleton->GetReferenceSkeleton().GetNum(); ++Index) { BoneIndices.Add(Index); }
            Proxy.GetRequiredBones().InitializeTo(BoneIndices, UE::Anim::FCurveFilterSettings(), *Skeleton);
        }
        Proxy.InitializeGraph();
        FProbeBlend Blend;
        FProbeTwoWay TwoWay;
        FProbeTwoWay Outer;
        FProbeLeaf Nested[2];
        FProbeCache Cache;
        FProbeLeaf CachedLeaf;
        TArray<int32> Order;
        TArray<FProbeLeaf> Leaves;
        Leaves.SetNum(Times->Num());
        if (bCapturePose)
        {
            for (int32 Index = 0; Index < Leaves.Num(); ++Index) { if (!ReadSourcePoseProbeLeaf(Skeleton->GetReferenceSkeleton(), (*Poses)[Index], Leaves[Index].Pose)) { return nullptr; } }
        }
        for (int32 Index = 0; Index < Leaves.Num(); ++Index) { Leaves[Index].Order = &Order; Leaves[Index].Index = Index; }
        if (TwoWayTemplate)
        {
            TwoWay.Configure(*TwoWayTemplate);
            TwoWay.A.SetLinkNode(&Nested[0]);
            TwoWay.B.SetLinkNode(&Nested[1]);
            Leaves[TwoWayChild].Forward = &TwoWay;
            Leaves[TwoWayChild].Order = nullptr;
            for (int32 Index = 0; Index < 2; ++Index) { Nested[Index].Order = &Order; Nested[Index].Index = Times->Num() + Index; }
        }
        if (CachedChild != INDEX_NONE)
        {
            CachedLeaf.Order = &Order;
            CachedLeaf.Index = CachedChild;
            Cache.Pose.SetLinkNode(&CachedLeaf);
            Leaves[CachedChild].Forward = &Cache;
            Leaves[CachedChild].Order = nullptr;
        }
        Blend.Link(Leaves);
        FArrayProperty* TimeProperty = FindFProperty<FArrayProperty>(FAnimNode_BlendListBase::StaticStruct(), TEXT("BlendTime"));
        FScriptArrayHelper TimeValues(TimeProperty, TimeProperty->ContainerPtrToValuePtr<void>(&Blend));
        TimeValues.Resize(Times->Num());
        const FFloatProperty* TimeValue = CastFieldChecked<FFloatProperty>(TimeProperty->Inner);
        for (int32 Index = 0; Index < Times->Num(); ++Index)
        {
            double Time;
            if (!(*Times)[Index]->TryGetNumber(Time) || !FMath::IsFinite(static_cast<float>(Time)) || Time < 0.0 || Time > 60.0) { return nullptr; }
            TimeValue->SetPropertyValue(TimeValues.GetRawPtr(Index), static_cast<float>(Time));
        }
        FEnumProperty* ModeProperty = FindFProperty<FEnumProperty>(FAnimNode_BlendListBase::StaticStruct(), TEXT("BlendType"));
        ModeProperty->GetUnderlyingProperty()->SetIntPropertyValue(ModeProperty->ContainerPtrToValuePtr<void>(&Blend), Mode);
        if (OuterTemplate)
        {
            Outer.Configure(*OuterTemplate);
            Outer.A.SetLinkNode(&Blend);
            Outer.B.SetLinkNode(&Cache);
            Outer.Initialize_AnyThread(FAnimationInitializeContext(&Proxy));
        }
        else { Blend.Initialize_AnyThread(FAnimationInitializeContext(&Proxy)); }
        if (bCapturePose) { Blend.CacheBones_AnyThread(FAnimationCacheBonesContext(&Proxy)); }
        TArray<TSharedPtr<FJsonValue>> Frames;
        for (const TSharedPtr<FJsonValue>& StepValue : *Steps)
        {
            const TSharedPtr<FJsonObject>* Step = nullptr;
            double Active, Delta;
            if (!StepValue->TryGetObject(Step) || !(*Step)->TryGetNumberField(TEXT("active"), Active) || !FMath::IsFinite(Active) ||
                Active < 0.0 || Active >= Times->Num() || Active != FMath::FloorToDouble(Active) ||
                !(*Step)->TryGetNumberField(TEXT("delta_seconds"), Delta) || !FMath::IsFinite(static_cast<float>(Delta)) || Delta < 0.0 || Delta > 1.0) { return nullptr; }
            double ParentWeight = 1.0;
            if ((*Step)->HasField(TEXT("parent_weight")) &&
                (!(*Step)->TryGetNumberField(TEXT("parent_weight"), ParentWeight) || !FMath::IsFinite(static_cast<float>(ParentWeight)) ||
                    ParentWeight < 0.0 || ParentWeight > 1.0)) { return nullptr; }
            const FAnimationUpdateContext UpdateContext = FAnimationUpdateContext(&Proxy, static_cast<float>(Delta)).FractionalWeight(static_cast<float>(ParentWeight));
            for (FProbeLeaf& Leaf : Leaves) { Leaf.Weights.Reset(); }
            CachedLeaf.Weights.Reset();
            Cache.Requests.Reset();
            Order.Reset();
            if (TwoWayTemplate)
            {
                double Alpha;
                if (!(*Step)->TryGetNumberField(TEXT("two_way_alpha"), Alpha) || !FMath::IsFinite(static_cast<float>(Alpha))) { return nullptr; }
                TwoWay.Alpha = static_cast<float>(Alpha);
                for (FProbeLeaf& Leaf : Nested) { Leaf.Weights.Reset(); }
            }
            Blend.Active = static_cast<int32>(Active);
            if (OuterTemplate)
            {
                double Alpha;
                if (!(*Step)->TryGetNumberField(TEXT("outer_alpha"), Alpha) || !FMath::IsFinite(static_cast<float>(Alpha))) { return nullptr; }
                Outer.Alpha = static_cast<float>(Alpha);
                Outer.Update_AnyThread(UpdateContext);
            }
            else { Blend.Update_AnyThread(UpdateContext); }
            if (CachedChild != INDEX_NONE) { Cache.PostGraphUpdate(); }
            TSharedRef<FJsonObject> Frame = MakeShared<FJsonObject>();
            Frame->SetNumberField(TEXT("active"), Blend.Active);
            Frame->SetNumberField(TEXT("duration"), Blend.Duration());
            TArray<TSharedPtr<FJsonValue>> Weights, Updates, Initializations;
            for (int32 Index = 0; Index < Leaves.Num(); ++Index)
            {
                Weights.Add(MakeShared<FJsonValueNumber>(Blend.Weight(Index)));
                Initializations.Add(MakeShared<FJsonValueNumber>(Leaves[Index].Initializations));
                TArray<TSharedPtr<FJsonValue>> Calls;
                for (const float Weight : Leaves[Index].Weights) { Calls.Add(MakeShared<FJsonValueNumber>(Weight)); }
                Updates.Add(MakeShared<FJsonValueArray>(Calls));
            }
            Frame->SetArrayField(TEXT("weights"), Weights);
            Frame->SetArrayField(TEXT("updates"), Updates);
            Frame->SetArrayField(TEXT("initializations"), Initializations);
            if (bCapturePose)
            {
                TArray<TSharedPtr<FJsonValue>> Inputs;
                for (FProbeLeaf& Leaf : Leaves)
                {
                    FPoseContext Input(&Proxy);
                    Leaf.Evaluate_AnyThread(Input);
                    const TSharedPtr<FJsonObject> Captured = CaptureCompactPose(Input.Pose, Input.Curve);
                    if (!Captured.IsValid()) { return nullptr; }
                    Inputs.Add(MakeShared<FJsonValueObject>(Captured));
                }
                FPoseContext Output(&Proxy);
                Blend.Evaluate_AnyThread(Output);
                const TSharedPtr<FJsonObject> Captured = CaptureCompactPose(Output.Pose, Output.Curve);
                if (!Captured.IsValid()) { return nullptr; }
                Frame->SetArrayField(TEXT("input_poses"), Inputs);
                Frame->SetObjectField(TEXT("pose"), Captured);
            }
            if (OuterTemplate)
            {
                Frame->SetNumberField(TEXT("outer_alpha"), Outer.Value());
                TArray<TSharedPtr<FJsonValue>> Requests;
                for (const float Weight : Cache.Requests) { Requests.Add(MakeShared<FJsonValueNumber>(Weight)); }
                Frame->SetArrayField(TEXT("cached_requests"), Requests);
            }
            if (TwoWayTemplate || CachedChild != INDEX_NONE)
            {
                TArray<TSharedPtr<FJsonValue>> Values;
                for (const int32 Index : Order) { Values.Add(MakeShared<FJsonValueNumber>(Index)); }
                Frame->SetArrayField(TEXT("update_order"), Values);
            }
            if (CachedChild != INDEX_NONE)
            {
                TArray<TSharedPtr<FJsonValue>> Calls;
                for (const float Weight : CachedLeaf.Weights) { Calls.Add(MakeShared<FJsonValueNumber>(Weight)); }
                Frame->SetArrayField(TEXT("cached_updates"), Calls);
                Frame->SetNumberField(TEXT("cached_initializations"), CachedLeaf.Initializations);
            }
            if (TwoWayTemplate)
            {
                TSharedRef<FJsonObject> Child = MakeShared<FJsonObject>();
                Child->SetNumberField(TEXT("alpha"), TwoWay.Value());
                TArray<TSharedPtr<FJsonValue>> ChildUpdates, ChildInitializations;
                for (const FProbeLeaf& Leaf : Nested)
                {
                    TArray<TSharedPtr<FJsonValue>> Calls;
                    for (const float Weight : Leaf.Weights) { Calls.Add(MakeShared<FJsonValueNumber>(Weight)); }
                    ChildUpdates.Add(MakeShared<FJsonValueArray>(Calls));
                    ChildInitializations.Add(MakeShared<FJsonValueNumber>(Leaf.Initializations));
                }
                Child->SetArrayField(TEXT("updates"), ChildUpdates);
                Child->SetArrayField(TEXT("initializations"), ChildInitializations);
                Frame->SetObjectField(TEXT("two_way"), Child);
            }
            Frames.Add(MakeShared<FJsonValueObject>(Frame));
        }
        TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
        Row->SetStringField(TEXT("name"), Name);
        Row->SetArrayField(TEXT("frames"), Frames);
        Rows.Add(MakeShared<FJsonValueObject>(Row));
        UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Blend list sample %d/%d captured."), Rows.Num(), Samples->Num());
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetArrayField(TEXT("samples"), Rows);
    return Result;
}

TSharedPtr<FJsonObject> CaptureBlendProfileProbe(UObject* Asset, const FString& RequestPath)
{
    FMemMark Mark(FMemStack::Get());
    USkeleton* Skeleton = Cast<USkeleton>(Asset);
    FString Text;
    TSharedPtr<FJsonObject> Request;
    const TArray<TSharedPtr<FJsonValue>>* Samples = nullptr;
    if (Skeleton == nullptr || !FFileHelper::LoadFileToString(Text, *RequestPath) ||
        !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) ||
        !Request->TryGetArrayField(TEXT("samples"), Samples) || Samples->IsEmpty() || Samples->Num() > 512) { return nullptr; }
    const auto ReadTransform = [](const TSharedPtr<FJsonObject>& Object, const TCHAR* Key, FTransform& Transform)
    {
        const TSharedPtr<FJsonObject>* Fields = nullptr;
        if (!Object->TryGetObjectField(Key, Fields)) { return false; }
        double Components[10];
        int32 Offset = 0;
        for (const TCHAR* Field : {TEXT("translation_cm"), TEXT("rotation"), TEXT("scale")})
        {
            const TArray<TSharedPtr<FJsonValue>>* Values = nullptr;
            const int32 Count = Offset == 3 ? 4 : 3;
            if (!(*Fields)->TryGetArrayField(Field, Values) || Values->Num() != Count) { return false; }
            for (const TSharedPtr<FJsonValue>& Value : *Values)
            {
                if (!Value->TryGetNumber(Components[Offset]) || !FMath::IsFinite(Components[Offset])) { return false; }
                ++Offset;
            }
        }
        const FQuat Rotation(Components[3], Components[4], Components[5], Components[6]);
        if (!Rotation.IsNormalized()) { return false; }
        Transform = FTransform(Rotation, FVector(Components[0], Components[1], Components[2]), FVector(Components[7], Components[8], Components[9]));
        return true;
    };
    TArray<TSharedPtr<FJsonValue>> Results;
    for (const TSharedPtr<FJsonValue>& Value : *Samples)
    {
        const TSharedPtr<FJsonObject>* Sample = nullptr;
        const TArray<TSharedPtr<FJsonValue>>* Deltas = nullptr;
        FString Name, ProfileName, ModeName;
        double Duration, CurveA, CurveB;
        FTransform From, To;
        if (!Value->TryGetObject(Sample) || !(*Sample)->TryGetStringField(TEXT("name"), Name) ||
            !(*Sample)->TryGetStringField(TEXT("profile"), ProfileName) || !(*Sample)->TryGetStringField(TEXT("mode"), ModeName) ||
            !(*Sample)->TryGetNumberField(TEXT("duration"), Duration) || !FMath::IsFinite(static_cast<float>(Duration)) || static_cast<float>(Duration) <= 0.0f ||
            !(*Sample)->TryGetNumberField(TEXT("curve_a"), CurveA) || !FMath::IsFinite(static_cast<float>(CurveA)) ||
            !(*Sample)->TryGetNumberField(TEXT("curve_b"), CurveB) || !FMath::IsFinite(static_cast<float>(CurveB)) ||
            !(*Sample)->TryGetArrayField(TEXT("deltas"), Deltas) || Deltas->IsEmpty() || Deltas->Num() > 256 ||
            !ReadTransform(*Sample, TEXT("from"), From) || !ReadTransform(*Sample, TEXT("to"), To)) { return nullptr; }
        UBlendProfile* Profile = Skeleton->GetBlendProfile(FName(*ProfileName));
        const int64 ModeValue = StaticEnum<EAlphaBlendOption>()->GetValueByNameString(ModeName);
        if (Profile == nullptr || Profile->Mode != EBlendProfileMode::WeightFactor || ModeValue == INDEX_NONE) { return nullptr; }
        const EAlphaBlendOption Mode = static_cast<EAlphaBlendOption>(ModeValue);
        UCurveFloat* CustomCurve = nullptr;
        if (Mode == EAlphaBlendOption::Custom)
        {
            FString CurvePath;
            if (!(*Sample)->TryGetStringField(TEXT("custom_curve"), CurvePath)) { return nullptr; }
            CustomCurve = LoadObject<UCurveFloat>(nullptr, *CurvePath);
            if (CustomCurve == nullptr) { return nullptr; }
        }
        FAlphaBlend Blend;
        Blend.SetBlendTime(static_cast<float>(Duration));
        Blend.SetBlendOption(Mode);
        Blend.SetCustomCurve(CustomCurve);
        Blend.SetValueRange(0.0f, 1.0f);
        TArray<FBlendSampleData> Weights;
        Weights.SetNum(2);
        for (FBlendSampleData& Weight : Weights) { Weight.PerBoneBlendData.SetNumZeroed(Profile->GetNumBlendEntries()); }
        FBlendedCurve PreviousCurve, NextCurve;
        PreviousCurve.Set(TEXT("ProbeCurve"), static_cast<float>(CurveA));
        NextCurve.Set(TEXT("ProbeCurve"), static_cast<float>(CurveB));
        float Elapsed = 0.0f;
        TArray<TSharedPtr<FJsonValue>> Frames;
        for (const TSharedPtr<FJsonValue>& DeltaValue : *Deltas)
        {
            double Delta;
            if (!DeltaValue->TryGetNumber(Delta) || !FMath::IsFinite(static_cast<float>(Delta)) || Delta < 0.0) { return nullptr; }
            Elapsed += static_cast<float>(Delta);
            if (!FMath::IsFinite(Elapsed)) { return nullptr; }
            Blend.Update(static_cast<float>(Delta));
            const float Factor = Elapsed / static_cast<float>(Duration);
            if (!FMath::IsFinite(Factor)) { return nullptr; }
            const float Alpha = FAlphaBlend::AlphaToBlendOption(Factor, Mode, CustomCurve);
            for (int32 Index = 0; Index < 2; ++Index)
            {
                Weights[Index].TotalWeight = Index == 0 ? Alpha : 1.0f - Alpha;
                Profile->UpdateBoneWeights(Weights[Index], Blend, 0.0f, Weights[Index].TotalWeight, Index != 0);
            }
            FBlendSampleData::NormalizeDataWeight(Weights);
            TSharedRef<FJsonObject> Bones = MakeShared<FJsonObject>();
            for (int32 Index = -1; Index < Profile->GetNumBlendEntries(); ++Index)
            {
                const float First = Index < 0 ? 1.0f - Alpha : Weights[1].PerBoneBlendData[Index];
                const float Second = Index < 0 ? Alpha : Weights[0].PerBoneBlendData[Index];
                FTransform Output = From * ScalarRegister(First);
                Output.AccumulateWithShortestRotation(To, ScalarRegister(Second));
                const FTransform RawOutput = Output;
                Output.NormalizeRotation();
                TSharedRef<FJsonObject> Bone = MakeShared<FJsonObject>();
                Bone->SetNumberField(TEXT("source_weight"), First);
                Bone->SetNumberField(TEXT("target_weight"), Second);
                Bone->SetObjectField(TEXT("raw_transform"), CaptureTransform(RawOutput));
                Bone->SetObjectField(TEXT("transform"), CaptureTransform(Output));
                Bones->SetObjectField(Index < 0 ? TEXT("default") : Profile->ProfileEntries[Index].BoneReference.BoneName.ToString(), Bone);
            }
            FBlendedCurve Curve;
            Curve.Override(PreviousCurve, 1.0 - Alpha);
            Curve.Accumulate(NextCurve, Alpha);
            FBlendedCurve TwoWayCurve;
            const float WeightOfPoseOne = 1.f - Alpha;
            const float WeightOfPoseTwo = 1.f - WeightOfPoseOne;
            TwoWayCurve.Lerp(PreviousCurve, NextCurve, WeightOfPoseTwo);
            TSharedRef<FJsonObject> Frame = MakeShared<FJsonObject>();
            Frame->SetNumberField(TEXT("elapsed"), Elapsed);
            Frame->SetNumberField(TEXT("factor"), Factor);
            Frame->SetNumberField(TEXT("alpha"), Alpha);
            Frame->SetNumberField(TEXT("blend_alpha"), Blend.GetAlpha());
            Frame->SetNumberField(TEXT("blend_value"), Blend.GetBlendedValue());
            Frame->SetBoolField(TEXT("complete"), Blend.IsComplete());
            Frame->SetNumberField(TEXT("curve"), Curve.Get(TEXT("ProbeCurve")));
            Frame->SetNumberField(TEXT("two_way_curve"), TwoWayCurve.Get(TEXT("ProbeCurve")));
            Frame->SetNumberField(TEXT("two_way_effective_alpha"), WeightOfPoseTwo);
            if ((*Sample)->HasField(TEXT("weighted_inputs")))
            {
                const TArray<TSharedPtr<FJsonValue>>* Inputs = nullptr;
                FString PoseMode;
                if (!(*Sample)->TryGetArrayField(TEXT("weighted_inputs"), Inputs) || Inputs->IsEmpty() || Inputs->Num() > 8 ||
                    !(*Sample)->TryGetStringField(TEXT("pose_mode"), PoseMode) ||
                    (PoseMode != TEXT("multi") && PoseMode != TEXT("two") && PoseMode != TEXT("gait"))) { return nullptr; }
                TArray<FBoneIndexType> RequiredIndices = {0};
                FBoneContainer Required;
                Required.InitializeTo(RequiredIndices, UE::Anim::FCurveFilterSettings(), *Skeleton);
                TArray<FCompactPose> InputPoses;
                TArray<float> InputWeights;
                for (const TSharedPtr<FJsonValue>& InputValue : *Inputs)
                {
                    const TSharedPtr<FJsonObject>* Input = nullptr;
                    double Weight;
                    FTransform Transform;
                    if (!InputValue->TryGetObject(Input) || !(*Input)->HasTypedField<EJson::Number>(TEXT("weight")) || !(*Input)->TryGetNumberField(TEXT("weight"), Weight) ||
                        !FMath::IsFinite(static_cast<float>(Weight)) || static_cast<float>(Weight) <= 0.f || Weight > 1.0 || !ReadTransform(*Input, TEXT("transform"), Transform)) { return nullptr; }
                    FCompactPose& InputPose = InputPoses.AddDefaulted_GetRef();
                    InputPose.SetBoneContainer(&Required);
                    InputPose.ResetToRefPose();
                    InputPose[FCompactPoseBoneIndex(0)] = Transform;
                    InputWeights.Add(static_cast<float>(Weight));
                }
                FCompactPose OutputPose;
                FBlendedCurve OutputCurve;
                UE::Anim::FStackAttributeContainer OutputAttributes;
                OutputPose.SetBoneContainer(&Required);
                OutputCurve.InitFrom(Required);
                FAnimationPoseData OutputData(OutputPose, OutputCurve, OutputAttributes);
                if (PoseMode == TEXT("two") && InputPoses.Num() != 2) { return nullptr; }
                if (InputPoses.Num() == 2 && (PoseMode == TEXT("two") ||
                    (PoseMode == TEXT("gait") && FMath::IsNearlyEqual(InputWeights[0] + InputWeights[1], 1.f))))
                {
                    FBlendedCurve EmptyA, EmptyB;
                    EmptyA.InitFrom(Required); EmptyB.InitFrom(Required);
                    UE::Anim::FStackAttributeContainer AttributesA, AttributesB;
                    FAnimationPoseData A(InputPoses[0], EmptyA, AttributesA), B(InputPoses[1], EmptyB, AttributesB);
                    FAnimationRuntime::BlendTwoPosesTogether(A, B, InputWeights[0], OutputData);
                }
                else if (PoseMode == TEXT("gait") && InputPoses.Num() == 1 && FAnimWeight::IsFullWeight(InputWeights[0]))
                {
                    OutputPose.CopyBonesFrom(InputPoses[0]);
                }
                else
                {
                    FAnimationRuntime::BlendPosesTogether(InputPoses, {}, {}, InputWeights, OutputData);
                    if (PoseMode == TEXT("multi")) { OutputPose.NormalizeRotations(); }
                }
                Frame->SetObjectField(TEXT("weighted_pose"), CaptureTransform(OutputPose[FCompactPoseBoneIndex(0)]));
            }
            Frame->SetObjectField(TEXT("bones"), Bones);
            Frames.Add(MakeShared<FJsonValueObject>(Frame));
        }
        TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
        Row->SetStringField(TEXT("name"), Name);
        if (CustomCurve != nullptr)
        {
            Row->SetStringField(TEXT("custom_curve"), CustomCurve->GetPathName());
            TArray<TSharedPtr<FJsonValue>> Keys;
            for (const FRichCurveKey& Key : CustomCurve->FloatCurve.GetConstRefOfKeys())
            {
                TSharedRef<FJsonObject> Entry = MakeShared<FJsonObject>();
                Entry->SetNumberField(TEXT("time"), Key.Time);
                Entry->SetNumberField(TEXT("value"), Key.Value);
                Entry->SetNumberField(TEXT("arrive_tangent"), Key.ArriveTangent);
                Entry->SetNumberField(TEXT("leave_tangent"), Key.LeaveTangent);
                Keys.Add(MakeShared<FJsonValueObject>(Entry));
            }
            Row->SetArrayField(TEXT("custom_curve_keys"), Keys);
        }
        Row->SetArrayField(TEXT("frames"), Frames);
        Results.Add(MakeShared<FJsonValueObject>(Row));
        UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Blend profile probe sample %d/%d captured."), Results.Num(), Samples->Num());
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetArrayField(TEXT("samples"), Results);
    return Result;
}

TSharedPtr<FJsonObject> CaptureLayeredBoneBlendProbe(UObject* Asset, const FString& RequestPath)
{
    FMemMark Mark(FMemStack::Get());
    UBlueprint* Blueprint = Cast<UBlueprint>(Asset);
    UAnimBlueprintGeneratedClass* Class = Blueprint != nullptr ? Cast<UAnimBlueprintGeneratedClass>(Blueprint->GeneratedClass) : nullptr;
    const IAnimClassInterface* Interface = Class != nullptr ? IAnimClassInterface::GetFromClass(Class) : nullptr;
    FString Text, NodeGuidText;
    TSharedPtr<FJsonObject> Request;
    const TArray<TSharedPtr<FJsonValue>>* Names = nullptr;
    const TArray<TSharedPtr<FJsonValue>>* Samples = nullptr;
    FGuid NodeGuid;
    if (Interface == nullptr || Interface->GetTargetSkeleton() == nullptr || !FFileHelper::LoadFileToString(Text, *RequestPath) ||
        !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) ||
        !Request->TryGetStringField(TEXT("node_guid"), NodeGuidText) || !FGuid::Parse(NodeGuidText, NodeGuid) || !NodeGuid.IsValid() ||
        !Request->TryGetArrayField(TEXT("bone_names"), Names) || !Request->TryGetArrayField(TEXT("samples"), Samples) ||
        Samples->IsEmpty() || Samples->Num() > 256) { return nullptr; }
    const int32* NodeIndex = Class->GetNodePropertyIndexFromGuid(NodeGuid);
    const TArray<FStructProperty*>& Properties = Interface->GetAnimNodeProperties();
    const int32 PropertyIndex = NodeIndex != nullptr ? Properties.Num() - 1 - *NodeIndex : INDEX_NONE;
    if (!Properties.IsValidIndex(PropertyIndex) || Properties[PropertyIndex]->Struct != FAnimNode_LayeredBoneBlend::StaticStruct()) { return nullptr; }
    const UEdGraphNode* Visual = Class->GetVisualNodeFromNodePropertyIndex(PropertyIndex);
    if (Visual == nullptr || Visual->NodeGuid != NodeGuid) { return nullptr; }
    const FAnimNode_LayeredBoneBlend* Template = Properties[PropertyIndex]->ContainerPtrToValuePtr<FAnimNode_LayeredBoneBlend>(Class->GetDefaultObject());
    USkeleton* Skeleton = Interface->GetTargetSkeleton();
    const FReferenceSkeleton& Reference = Skeleton->GetReferenceSkeleton();
    if (Names->Num() != Reference.GetNum()) { return nullptr; }
    TArray<FBoneIndexType> BoneIndices;
    for (int32 Index = 0; Index < Names->Num(); ++Index)
    {
        FString Name;
        if (!(*Names)[Index]->TryGetString(Name) || Name != Reference.GetBoneName(Index).ToString()) { return nullptr; }
        BoneIndices.Add(Index);
    }
    struct FProbeProxy : FAnimInstanceProxy
    {
        using FAnimInstanceProxy::FAnimInstanceProxy;
        using FAnimInstanceProxy::Initialize;
    };
    struct FProbeLayered : FAnimNode_LayeredBoneBlend
    {
        const TArray<FPerBoneBlendWeight>& Weights() const { return CurrentBoneBlendWeights; }
        TSharedRef<FJsonObject> CurveSources() const
        {
            TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
            CurvePoseSourceIndices.ForEachElement([&Result](const UE::Anim::FCurveElementIndexed& Curve) { Result->SetNumberField(Curve.Name.ToString(), Curve.Index); });
            return Result;
        }
    };
    TArray<TSharedPtr<FJsonValue>> Results;
    for (const TSharedPtr<FJsonValue>& Value : *Samples)
    {
        const TSharedPtr<FJsonObject>* Sample = nullptr;
        const TArray<TSharedPtr<FJsonValue>>* Layers = nullptr;
        FString Name;
        double Delta;
        if (!Value->TryGetObject(Sample) || !(*Sample)->TryGetStringField(TEXT("name"), Name) ||
            !(*Sample)->HasTypedField<EJson::Number>(TEXT("delta_seconds")) || !(*Sample)->TryGetNumberField(TEXT("delta_seconds"), Delta) ||
            !FMath::IsFinite(static_cast<float>(Delta)) || Delta < 0.0 ||
            !(*Sample)->TryGetArrayField(TEXT("layers"), Layers) || Layers->Num() != Template->BlendPoses.Num() || !(*Sample)->HasField(TEXT("base"))) { return nullptr; }
        TArray<FSourcePoseProbeLeaf> Leaves;
        Leaves.SetNum(Layers->Num() + 1);
        if (!ReadSourcePoseProbeLeaf(Reference, (*Sample)->TryGetField(TEXT("base")), Leaves[0])) { return nullptr; }
        for (int32 Index = 0; Index < Layers->Num(); ++Index) { if (!ReadSourcePoseProbeLeaf(Reference, (*Layers)[Index], Leaves[Index + 1])) { return nullptr; } }
        USkeletalMeshComponent* Mesh = NewObject<USkeletalMeshComponent>();
        UAnimInstance* Instance = NewObject<UAnimInstance>(Mesh, Class);
        FProbeProxy Proxy(Instance);
        Proxy.Initialize(Instance);
        if (!Proxy.IsSkeletonValid() || Proxy.GetSkeleton() != Skeleton) { return nullptr; }
        Proxy.GetRequiredBones().InitializeTo(BoneIndices, UE::Anim::FCurveFilterSettings(), *Skeleton);
        FProbeLayered Node;
        static_cast<FAnimNode_LayeredBoneBlend&>(Node) = *Template;
        Node.InvalidatePerBoneBlendWeights();
        Node.BasePose.SetLinkNode(&Leaves[0]);
        for (int32 Index = 0; Index < Layers->Num(); ++Index) { Node.BlendPoses[Index].SetLinkNode(&Leaves[Index + 1]); }
        Node.Initialize_AnyThread(FAnimationInitializeContext(&Proxy));
        Node.CacheBones_AnyThread(FAnimationCacheBonesContext(&Proxy));
        FAnimationUpdateSharedContext SharedContext;
        Node.Update_AnyThread(FAnimationUpdateContext(&Proxy, static_cast<float>(Delta), &SharedContext));
        FPoseContext Pose(&Proxy);
        Node.Evaluate_AnyThread(Pose);
        TArray<TSharedPtr<FJsonValue>> Transforms, Weights, UpdateWeights, RootMotionWeights;
        for (int32 Index = 0; Index < BoneIndices.Num(); ++Index)
        {
            if (Pose.Pose[FCompactPoseBoneIndex(Index)].ContainsNaN() || !Pose.Pose[FCompactPoseBoneIndex(Index)].GetRotation().IsNormalized()) { return nullptr; }
            Transforms.Add(MakeShared<FJsonValueObject>(CaptureTransform(Pose.Pose[FCompactPoseBoneIndex(Index)])));
            TSharedRef<FJsonObject> Weight = MakeShared<FJsonObject>();
            Weight->SetNumberField(TEXT("source"), Node.Weights()[Index].SourceIndex);
            Weight->SetNumberField(TEXT("weight"), Node.Weights()[Index].BlendWeight);
            Weights.Add(MakeShared<FJsonValueObject>(Weight));
        }
        for (const FSourcePoseProbeLeaf& Leaf : Leaves)
        {
            UpdateWeights.Add(MakeShared<FJsonValueNumber>(Leaf.UpdateWeight));
            RootMotionWeights.Add(MakeShared<FJsonValueNumber>(Leaf.RootMotionWeight));
        }
        TSharedRef<FJsonObject> Curves = MakeShared<FJsonObject>();
        Pose.Curve.ForEachElement([&Curves](const UE::Anim::FCurveElement& Curve) { Curves->SetNumberField(Curve.Name.ToString(), Curve.Value); });
        TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
        Row->SetStringField(TEXT("name"), Name);
        Row->SetArrayField(TEXT("bone_transforms"), Transforms);
        Row->SetArrayField(TEXT("bone_weights"), Weights);
        Row->SetArrayField(TEXT("update_weights"), UpdateWeights);
        Row->SetArrayField(TEXT("root_motion_weight_modifiers"), RootMotionWeights);
        Row->SetObjectField(TEXT("curve_sources"), Node.CurveSources());
        Row->SetObjectField(TEXT("curves"), Curves);
        Results.Add(MakeShared<FJsonValueObject>(Row));
        UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Layered bone blend sample %d/%d captured."), Results.Num(), Samples->Num());
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetStringField(TEXT("node_guid"), GuidText(NodeGuid));
    Result->SetStringField(TEXT("node_property"), Properties[PropertyIndex]->GetName());
    Result->SetArrayField(TEXT("samples"), Results);
    return Result;
}

TSharedPtr<FJsonObject> CaptureRigPoseTransferProbe(UObject* Asset, const FString& RequestPath)
{
    FMemMark Mark(FMemStack::Get());
    UBlueprint* Blueprint = Cast<UBlueprint>(Asset);
    UAnimBlueprintGeneratedClass* Class = Blueprint != nullptr ? Cast<UAnimBlueprintGeneratedClass>(Blueprint->GeneratedClass) : nullptr;
    const IAnimClassInterface* Interface = Class != nullptr ? IAnimClassInterface::GetFromClass(Class) : nullptr;
    FString Text, GuidTextValue, MeshPath;
    FGuid Guid;
    TSharedPtr<FJsonObject> Request;
    const TArray<TSharedPtr<FJsonValue>>* Names = nullptr;
    const TArray<TSharedPtr<FJsonValue>>* Samples = nullptr;
    if (Interface == nullptr || !FFileHelper::LoadFileToString(Text, *RequestPath) ||
        !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) ||
        !Request->TryGetStringField(TEXT("node_guid"), GuidTextValue) || !FGuid::Parse(GuidTextValue, Guid) || !Guid.IsValid() ||
        !Request->TryGetStringField(TEXT("mesh_asset"), MeshPath) || !Request->TryGetArrayField(TEXT("bone_names"), Names) ||
        !Request->TryGetArrayField(TEXT("samples"), Samples) || Samples->IsEmpty() || Samples->Num() > 240) { return nullptr; }
    USkeletalMesh* MeshAsset = LoadObject<USkeletalMesh>(nullptr, *MeshPath);
    if (MeshAsset == nullptr || MeshAsset->GetSkeleton() != Interface->GetTargetSkeleton()) { return nullptr; }
    const FReferenceSkeleton& Reference = MeshAsset->GetRefSkeleton();
    if (Names->Num() != Reference.GetNum()) { return nullptr; }
    TArray<FBoneIndexType> Bones;
    for (int32 Index = 0; Index < Names->Num(); ++Index)
    {
        FString Name;
        if (!(*Names)[Index]->TryGetString(Name) || Name != Reference.GetBoneName(Index).ToString()) { return nullptr; }
        Bones.Add(Index);
    }
    const int32* NodeIndex = Class->GetNodePropertyIndexFromGuid(Guid);
    const TArray<FStructProperty*>& Properties = Interface->GetAnimNodeProperties();
    const int32 Index = NodeIndex != nullptr ? Properties.Num() - 1 - *NodeIndex : INDEX_NONE;
    if (!Properties.IsValidIndex(Index) || Properties[Index]->Struct != FAnimNode_ControlRig::StaticStruct()) { return nullptr; }
    const UEdGraphNode* Visual = Class->GetVisualNodeFromNodePropertyIndex(Index);
    if (Visual == nullptr || Visual->NodeGuid != Guid) { return nullptr; }
    const FAnimNode_ControlRig* Template = Properties[Index]->ContainerPtrToValuePtr<FAnimNode_ControlRig>(Class->GetDefaultObject());
    struct FProbeNode : FAnimNode_ControlRig
    {
        using FAnimNode_ControlRig::UpdateInput;
        using FAnimNode_ControlRig::UpdateOutput;
        bool AdapterReady() const { return ControlRigHierarchyMappings.CheckPoseAdapter(); }
        bool AdapterEnabled() const { return ControlRigHierarchyMappings.IsPoseAdapterEnabled(); }
        void SetSource(FAnimNode_Base* Leaf) { Source.SetLinkNode(Leaf); }
        TSharedRef<FJsonObject> TransferFlags() const
        {
            TSharedRef<FJsonObject> Flags = MakeShared<FJsonObject>();
            Flags->SetBoolField(TEXT("input_pose"), bTransferInputPose);
            Flags->SetBoolField(TEXT("input_curves"), bTransferInputCurves);
            Flags->SetBoolField(TEXT("global_space"), bTransferPoseInGlobalSpace);
            Flags->SetBoolField(TEXT("reset_pose"), bResetInputPoseToInitial);
            return Flags;
        }
    };
    struct FProbeProxy : FAnimInstanceProxy { using FAnimInstanceProxy::FAnimInstanceProxy; using FAnimInstanceProxy::Initialize; };
    TArray<TSharedPtr<FJsonValue>> Results;
    for (const TSharedPtr<FJsonValue>& Value : *Samples)
    {
        const TSharedPtr<FJsonObject>* Sample = nullptr;
        FString Name;
        if (!Value->TryGetObject(Sample) || !(*Sample)->TryGetStringField(TEXT("name"), Name) || !(*Sample)->HasField(TEXT("pose"))) { return nullptr; }
        FSourcePoseProbeLeaf Leaf;
        if (!ReadSourcePoseProbeLeaf(Reference, (*Sample)->TryGetField(TEXT("pose")), Leaf)) { return nullptr; }
        USkeletalMeshComponent* Mesh = NewObject<USkeletalMeshComponent>();
        Mesh->SetSkeletalMeshAsset(MeshAsset);
        UAnimInstance* Instance = NewObject<UAnimInstance>(Mesh, Class);
        FProbeProxy Proxy(Instance);
        Proxy.Initialize(Instance);
        Proxy.GetRequiredBones().InitializeTo(Bones, UE::Anim::FCurveFilterSettings(), *MeshAsset);
        Instance->GetRequiredBones().InitializeTo(Bones, UE::Anim::FCurveFilterSettings(), *MeshAsset);
        FProbeNode Node;
        static_cast<FAnimNode_ControlRig&>(Node) = *Template;
        Node.SetSource(&Leaf);
        Node.OnInitializeAnimInstance(&Proxy, Instance);
        Node.Initialize_AnyThread(FAnimationInitializeContext(&Proxy));
        Node.CacheBones_AnyThread(FAnimationCacheBonesContext(&Proxy));
        UControlRig* Rig = Node.GetControlRig();
        if (Rig == nullptr || !Node.AdapterEnabled() || !Node.AdapterReady()) { return nullptr; }
        FPoseContext Pose(&Proxy);
        Leaf.Evaluate_AnyThread(Pose);
        TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
        Row->SetStringField(TEXT("name"), Name);
        Row->SetBoolField(TEXT("adapter_enabled"), Node.AdapterEnabled());
        Row->SetBoolField(TEXT("adapter_ready"), Node.AdapterReady());
        Row->SetNumberField(TEXT("compact_bones"), Pose.Pose.GetNumBones());
        Row->SetNumberField(TEXT("virtual_bones"), Proxy.GetRequiredBones().GetVirtualBoneCompactPoseData().Num());
        Row->SetObjectField(TEXT("transfer_flags"), Node.TransferFlags());
        const TSharedPtr<FJsonObject> Before = CaptureCompactPose(Pose.Pose, Pose.Curve);
        if (!Before.IsValid()) { return nullptr; }
        Row->SetObjectField(TEXT("before"), Before.ToSharedRef());
        Node.UpdateInput(Rig, Pose);
        TArray<TSharedPtr<FJsonValue>> Transferred;
        for (const FRigElementKey& Key : Rig->GetHierarchy()->GetBoneKeys())
        {
            TSharedRef<FJsonObject> Bone = MakeShared<FJsonObject>();
            Bone->SetStringField(TEXT("name"), Key.Name.ToString());
            Bone->SetObjectField(TEXT("local"), CaptureTransform(Rig->GetHierarchy()->GetLocalTransform(Key)));
            Transferred.Add(MakeShared<FJsonValueObject>(Bone));
        }
        Row->SetArrayField(TEXT("transferred"), Transferred);
        TArray<TSharedPtr<FJsonValue>> HierarchyCurves;
        for (const FRigCurveElement* Curve : Rig->GetHierarchy()->GetCurves())
        {
            TSharedRef<FJsonObject> CurveValue = MakeShared<FJsonObject>();
            CurveValue->SetStringField(TEXT("name"), Curve->GetFName().ToString());
            CurveValue->SetBoolField(TEXT("set"), Curve->IsValueSet());
            CurveValue->SetNumberField(TEXT("value"), Curve->Get());
            HierarchyCurves.Add(MakeShared<FJsonValueObject>(CurveValue));
        }
        Row->SetArrayField(TEXT("hierarchy_curves"), HierarchyCurves);
        Node.UpdateOutput(Rig, Pose);
        const TSharedPtr<FJsonObject> After = CaptureCompactPose(Pose.Pose, Pose.Curve);
        if (!After.IsValid()) { return nullptr; }
        Row->SetObjectField(TEXT("after"), After.ToSharedRef());
        Results.Add(MakeShared<FJsonValueObject>(Row));
        UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Rig pose transfer sample %d/%d."), Results.Num(), Samples->Num());
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetStringField(TEXT("node_guid"), GuidText(Guid));
    Result->SetStringField(TEXT("mesh_asset"), MeshAsset->GetPathName());
    Result->SetArrayField(TEXT("samples"), Results);
    return Result;
}

TSharedPtr<FJsonObject> CaptureCharacterAnimationProbe(UObject* Asset, const FString& RequestPath)
{
    UBlueprint* Blueprint = Cast<UBlueprint>(Asset);
    UClass* Class = Blueprint != nullptr ? Blueprint->GeneratedClass.Get() : Cast<UClass>(Asset);
    FString Text;
    double Gravity;
    TSharedPtr<FJsonObject> Request;
    const TArray<TSharedPtr<FJsonValue>>* Cases = nullptr;
    if (Class == nullptr || !Class->IsChildOf(ACharacter::StaticClass()) || !FFileHelper::LoadFileToString(Text, *RequestPath) ||
        !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) || !Request->TryGetNumberField(TEXT("gravity_cm_s2"), Gravity) || !FMath::IsFinite(static_cast<float>(Gravity)) || Gravity >= 0 || !Request->TryGetArrayField(TEXT("cases"), Cases) || Cases->IsEmpty() || Cases->Num() > 8) { return nullptr; }
    bool bCapturePoseBoundaries = false;
    if (Request->HasField(TEXT("capture_pose_boundaries")) &&
        (Request->TryGetField(TEXT("capture_pose_boundaries"))->Type != EJson::Boolean || !Request->TryGetBoolField(TEXT("capture_pose_boundaries"), bCapturePoseBoundaries))) { return nullptr; }
    struct FObservedPose : FAnimNode_Base
    {
        FPoseLink Source;
        TSharedPtr<FJsonObject> Latest;
        int32 Updates = 0;
        int32 Evaluations = 0;
        float Delta = 0;
        virtual void Initialize_AnyThread(const FAnimationInitializeContext& Context) override { Source.Initialize(Context); }
        virtual void CacheBones_AnyThread(const FAnimationCacheBonesContext& Context) override { Source.CacheBones(Context); }
        virtual void Update_AnyThread(const FAnimationUpdateContext& Context) override { ++Updates; Delta = Context.GetDeltaTime(); Source.Update(Context); }
        virtual void Evaluate_AnyThread(FPoseContext& Output) override
        {
            Source.Evaluate(Output);
            ++Evaluations;
            const TSharedPtr<FJsonObject> Pose = CaptureCompactPose(Output.Pose, Output.Curve);
            Latest.Reset();
            if (!Pose.IsValid()) { return; }
            Latest = MakeShared<FJsonObject>();
            Latest->SetObjectField(TEXT("pose"), Pose.ToSharedRef());
            Latest->SetNumberField(TEXT("updates"), Updates);
            Latest->SetNumberField(TEXT("evaluations"), Evaluations);
            Latest->SetNumberField(TEXT("engine_frame"), GFrameCounter);
            Latest->SetNumberField(TEXT("delta_seconds"), Delta);
            Latest->SetNumberField(TEXT("attribute_count"), Output.CustomAttributes.Num());
            Latest->SetStringField(TEXT("required_bones_asset"), ObjectPath(Output.Pose.GetBoneContainer().GetAsset()));
        }
    };
    struct FObservedRigAccess : FAnimNode_ControlRig
    {
        static FPoseLink& SourceLink(FAnimNode_ControlRig& Node) { return Node.*&FObservedRigAccess::Source; }
    };
    struct FInstanceAccess : UAnimInstance
    {
        static FAnimInstanceProxy& Proxy(UAnimInstance* Instance) { return *GetProxyOnGameThreadStatic<FAnimInstanceProxy>(Instance); }
    };
    struct FBlendSpaceAccess : FAnimNode_BlendSpacePlayerBase
    {
        static const TArray<FBlendSampleData>& Samples(const FAnimNode_BlendSpacePlayerBase& Node) { return Node.*&FBlendSpaceAccess::BlendSampleDataCache; }
    };
    UClass* HeadClass = nullptr;
    FStructProperty* HeadProperty = nullptr;
    UClass* BaseProviderClass = nullptr;
    FStructProperty* BaseProviderProperty = nullptr;
    UClass* MainClass = nullptr;
    if (bCapturePoseBoundaries)
    {
        const ACharacter* DefaultCharacter = Cast<ACharacter>(Class->GetDefaultObject());
        USkeletalMeshComponent* DefaultMesh = DefaultCharacter != nullptr ? DefaultCharacter->GetMesh() : nullptr;
        MainClass = DefaultMesh != nullptr ? DefaultMesh->GetAnimClass() : nullptr;
        const IAnimClassInterface* MainInterface = MainClass != nullptr ? IAnimClassInterface::GetFromClass(MainClass) : nullptr;
        if (MainInterface == nullptr) { return nullptr; }
        const TArray<FStructProperty*>& MainProperties = MainInterface->GetAnimNodeProperties();
        FAnimNode_ControlRig* DefaultRig = nullptr;
        for (const FStructProperty* Property : MainProperties)
        {
            if (Property->Struct != FAnimNode_ControlRig::StaticStruct()) { continue; }
            if (DefaultRig != nullptr) { return nullptr; }
            DefaultRig = Property->ContainerPtrToValuePtr<FAnimNode_ControlRig>(MainClass->GetDefaultObject());
        }
        const int32 SourceIndex = DefaultRig != nullptr ? FObservedRigAccess::SourceLink(*DefaultRig).LinkID : INDEX_NONE;
        if (!MainProperties.IsValidIndex(SourceIndex) || !MainProperties[SourceIndex]->Struct->IsChildOf(FAnimNode_LinkedAnimGraph::StaticStruct())) { return nullptr; }
        const FAnimNode_LinkedAnimGraph* Caller = MainProperties[SourceIndex]->ContainerPtrToValuePtr<FAnimNode_LinkedAnimGraph>(MainClass->GetDefaultObject());
        if (Caller->InputPoses.Num() != 1 || !MainProperties.IsValidIndex(Caller->InputPoses[0].LinkID)) { return nullptr; }
        BaseProviderProperty = MainProperties[Caller->InputPoses[0].LinkID];
        if (!BaseProviderProperty->Struct->IsChildOf(FAnimNode_LinkedAnimGraph::StaticStruct())) { return nullptr; }
        const FAnimNode_LinkedAnimGraph* BaseProvider = BaseProviderProperty->ContainerPtrToValuePtr<FAnimNode_LinkedAnimGraph>(MainClass->GetDefaultObject());
        if (BaseProvider->InputPoses.Num() < 1) { return nullptr; }
        BaseProviderClass = BaseProvider->InstanceClass.Get();
        if (BaseProviderClass == nullptr) { return nullptr; }
        HeadClass = Caller->InstanceClass.Get();
        const IAnimClassInterface* HeadInterface = HeadClass != nullptr ? IAnimClassInterface::GetFromClass(HeadClass) : nullptr;
        if (HeadInterface == nullptr || HeadInterface->GetAnimBlueprintFunctions().IsEmpty()) { return nullptr; }
        const FStructProperty* RootProperty = CastField<FStructProperty>(HeadInterface->GetAnimBlueprintFunctions()[0].OutputPoseNodeProperty);
        if (RootProperty == nullptr || RootProperty->Struct != FAnimNode_Root::StaticStruct()) { return nullptr; }
        const FAnimNode_Root* Root = RootProperty->ContainerPtrToValuePtr<FAnimNode_Root>(HeadClass->GetDefaultObject());
        const TArray<FStructProperty*>& HeadProperties = HeadInterface->GetAnimNodeProperties();
        if (!HeadProperties.IsValidIndex(Root->Result.LinkID)) { return nullptr; }
        HeadProperty = HeadProperties[Root->Result.LinkID];
        if (HeadProperty->Struct != FAnimNode_ApplyMeshSpaceAdditive::StaticStruct()) { return nullptr; }
    }
    const auto CaptureGetter = [](UAnimInstance* Instance) -> TSharedPtr<FJsonObject>
    {
        UFunction* Function = Instance->FindFunction(TEXT("GetControlRigInput"));
        if (Function == nullptr) { return nullptr; }
        const FStructProperty* Return = CastField<FStructProperty>(Function->GetReturnProperty());
        if (Return == nullptr) { return nullptr; }
        FStructOnScope Parameters(Function);
        Instance->ProcessEvent(Function, Parameters.GetStructMemory());
        TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
        Result->SetArrayField(TEXT("fields"), CaptureProperties(Return->Struct, Return->ContainerPtrToValuePtr<void>(Parameters.GetStructMemory()), Instance));
        return Result;
    };
    const auto CaptureState = [](UAnimInstance* Instance)
    {
        TArray<TSharedPtr<FJsonValue>> Fields;
        for (const TCHAR* Name : {TEXT("Settings"), TEXT("Character"), TEXT("PoseState"), TEXT("FeetState"), TEXT("SpineState"), TEXT("GroundedState"), TEXT("LocomotionState"), TEXT("LocomotionMode"), TEXT("Stance"), TEXT("MovementBase"), TEXT("InAirState"), TEXT("LayeringState"), TEXT("bPendingUpdate"), TEXT("DeltaTimeWithoutTimeDilation")})
        {
            const FProperty* Property = Instance->GetClass()->FindPropertyByName(FName(Name));
            if (Property == nullptr) { continue; }
            const void* Memory = Property->ContainerPtrToValuePtr<void>(Instance);
            TSharedRef<FJsonObject> Field = MakeShared<FJsonObject>();
            Field->SetStringField(TEXT("name"), Name);
            FString Exported;
            Property->ExportTextItem_Direct(Exported, Memory, nullptr, Instance, PPF_None);
            Field->SetStringField(TEXT("value"), Exported);
            if (const FStructProperty* Struct = CastField<FStructProperty>(Property)) { Field->SetArrayField(TEXT("fields"), CaptureProperties(Struct->Struct, Memory, Instance)); }
            if (const FNumericProperty* Numeric = CastField<FNumericProperty>(Property); Numeric != nullptr && Numeric->IsFloatingPoint()) { Field->SetNumberField(TEXT("numeric_value"), Numeric->GetFloatingPointPropertyValue(Memory)); }
            Fields.Add(MakeShared<FJsonValueObject>(Field));
        }
        return Fields;
    };
    const auto CaptureInstances = [BaseProviderClass](USkeletalMeshComponent* Mesh, UAnimInstance* Main)
    {
            TArray<TSharedPtr<FJsonValue>> Linked;
            TArray<UAnimInstance*> Mains = static_cast<const USkeletalMeshComponent*>(Mesh)->GetLinkedAnimInstances();
            Mains.Insert(Main, 0);
            for (UAnimInstance* Child : Mains)
            {
                TSharedRef<FJsonObject> ChildRow = MakeShared<FJsonObject>();
                const FAnimInstanceProxy& ChildProxy = FInstanceAccess::Proxy(Child);
                ChildRow->SetStringField(TEXT("class"), Child->GetClass()->GetPathName());
                ChildRow->SetBoolField(TEXT("main"), Child == Main);
                TArray<TSharedPtr<FJsonValue>> Machines, Players, LayeredBlends;
                if (const IAnimClassInterface* ChildInterface = IAnimClassInterface::GetFromClass(Child->GetClass()))
                {
                    for (const FStructProperty* Property : ChildInterface->GetAnimNodeProperties())
                    {
                        if (Child->GetClass() == BaseProviderClass && Property->Struct == FAnimNode_LayeredBoneBlend::StaticStruct())
                        {
                            const FAnimNode_LayeredBoneBlend* Blend = Property->ContainerPtrToValuePtr<FAnimNode_LayeredBoneBlend>(Child);
                            TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
                            Value->SetStringField(TEXT("property"), Property->GetName());
                            Value->SetBoolField(TEXT("has_relevant_poses"), Blend->bHasRelevantPoses);
                            Value->SetBoolField(TEXT("mesh_space_rotation"), Blend->bMeshSpaceRotationBlend);
                            TArray<TSharedPtr<FJsonValue>> Weights;
                            for (float Weight : Blend->BlendWeights) { Weights.Add(MakeShared<FJsonValueNumber>(Weight)); }
                            Value->SetArrayField(TEXT("weights"), Weights);
                            LayeredBlends.Add(MakeShared<FJsonValueObject>(Value));
                        }
                        if (Property->Struct->IsChildOf(FAnimNode_AssetPlayerBase::StaticStruct()))
                        {
                            const FAnimNode_AssetPlayerBase* Player = Property->ContainerPtrToValuePtr<FAnimNode_AssetPlayerBase>(Child);
                            UAnimationAsset* Animation = Player->GetAnimAsset();
                            TSharedRef<FJsonObject> PlayerRow = MakeShared<FJsonObject>();
                            PlayerRow->SetStringField(TEXT("property"), Property->GetName());
                            PlayerRow->SetStringField(TEXT("node_type"), Property->Struct->GetPathName());
                            PlayerRow->SetStringField(TEXT("asset"), ObjectPath(Animation));
                            PlayerRow->SetNumberField(TEXT("current_asset_time"), Player->GetCurrentAssetTime());
                            PlayerRow->SetNumberField(TEXT("cached_blend_weight"), Player->GetCachedBlendWeight());
                            const FBoneContainer& Bones = ChildProxy.GetRequiredBones();
                            PlayerRow->SetBoolField(TEXT("required_bones_valid"), Bones.IsValid());
                            if (const UAnimBlueprintGeneratedClass* AnimationClass = Cast<UAnimBlueprintGeneratedClass>(Child->GetClass()))
                            {
                                const int32 PropertyIndex = ChildInterface->GetAnimNodeProperties().IndexOfByKey(Property);
                                if (const UEdGraphNode* Visual = AnimationClass->GetVisualNodeFromNodePropertyIndex(PropertyIndex)) { PlayerRow->SetStringField(TEXT("node_guid"), GuidText(Visual->NodeGuid)); }
                            }
                            if (UAnimSequence* Sequence = Cast<UAnimSequence>(Animation); Sequence != nullptr && Bones.IsValid())
                            {
                                const FAnimExtractContext Context(Player->GetCurrentAssetTime(), false);
                                PlayerRow->SetStringField(TEXT("pose_mode_at_observation"), FSequencePoseAccess::UsesRawData(*Sequence, Bones, Context) ? TEXT("raw") : TEXT("compressed"));
                            }
                            if (Property->Struct->IsChildOf(FAnimNode_BlendSpacePlayerBase::StaticStruct()))
                            {
                                const FAnimNode_BlendSpacePlayerBase* Blend = Property->ContainerPtrToValuePtr<FAnimNode_BlendSpacePlayerBase>(Child);
                                TArray<TSharedPtr<FJsonValue>> Samples;
                                for (const FBlendSampleData& Sample : FBlendSpaceAccess::Samples(*Blend))
                                {
                                    TSharedRef<FJsonObject> SampleRow = MakeShared<FJsonObject>();
                                    SampleRow->SetNumberField(TEXT("index"), Sample.SampleDataIndex);
                                    const UBlendSpace* BlendSpace = Blend->GetBlendSpace();
                                    UAnimSequence* Sequence = BlendSpace != nullptr && BlendSpace->GetBlendSamples().IsValidIndex(Sample.SampleDataIndex) ? BlendSpace->GetBlendSamples()[Sample.SampleDataIndex].Animation.Get() : nullptr;
                                    const float Time = Sequence != nullptr ? FMath::Clamp<float>(Sample.Time, 0.f, Sequence->GetPlayLength()) : Sample.Time;
                                    SampleRow->SetStringField(TEXT("asset"), ObjectPath(Sequence));
                                    SampleRow->SetNumberField(TEXT("time"), Time);
                                    SampleRow->SetNumberField(TEXT("cached_time"), Sample.Time);
                                    SampleRow->SetNumberField(TEXT("weight"), Sample.GetClampedWeight());
                                    if (Sequence != nullptr && Bones.IsValid()) { SampleRow->SetStringField(TEXT("pose_mode_at_observation"), FSequencePoseAccess::UsesRawData(*Sequence, Bones, FAnimExtractContext(Time, false)) ? TEXT("raw") : TEXT("compressed")); }
                                    Samples.Add(MakeShared<FJsonValueObject>(SampleRow));
                                }
                                PlayerRow->SetArrayField(TEXT("blend_samples"), Samples);
                            }
                            Players.Add(MakeShared<FJsonValueObject>(PlayerRow));
                        }
                        if (Property->Struct != FAnimNode_StateMachine::StaticStruct()) { continue; }
                        const FAnimNode_StateMachine* Machine = Property->ContainerPtrToValuePtr<FAnimNode_StateMachine>(Child);
                        TSharedRef<FJsonObject> MachineRow = MakeShared<FJsonObject>();
                        MachineRow->SetStringField(TEXT("property"), Property->GetName());
                        MachineRow->SetStringField(TEXT("state"), Machine->GetCurrentStateName().ToString());
                        MachineRow->SetNumberField(TEXT("state_index"), Machine->GetCurrentState());
                        MachineRow->SetNumberField(TEXT("elapsed"), Machine->GetCurrentStateElapsedTime());
                        Machines.Add(MakeShared<FJsonValueObject>(MachineRow));
                    }
                }
                ChildRow->SetArrayField(TEXT("machines"), Machines);
                ChildRow->SetArrayField(TEXT("asset_players"), Players);
                if (Child->GetClass() == BaseProviderClass) { ChildRow->SetArrayField(TEXT("layered_blends"), LayeredBlends); }
                ChildRow->SetNumberField(TEXT("update_counter"), ChildProxy.GetUpdateCounter().Get());
                ChildRow->SetNumberField(TEXT("evaluation_counter"), ChildProxy.GetEvaluationCounter().Get());
                Linked.Add(MakeShared<FJsonValueObject>(ChildRow));
            }
        return Linked;
    };
    double InitialEngineFrame = 0;
    if (Request->HasField(TEXT("initial_engine_frame")) &&
        (!Request->TryGetNumberField(TEXT("initial_engine_frame"), InitialEngineFrame) || !FMath::IsFinite(InitialEngineFrame) ||
         InitialEngineFrame < 0 || InitialEngineFrame > 4294967295.0 || InitialEngineFrame != FMath::FloorToDouble(InitialEngineFrame))) { return nullptr; }
    const bool bOverrideRaw = Request->HasField(TEXT("force_raw_data"));
    bool bForceRaw = false;
    if (bOverrideRaw && (Request->TryGetField(TEXT("force_raw_data"))->Type != EJson::Boolean || !Request->TryGetBoolField(TEXT("force_raw_data"), bForceRaw))) { return nullptr; }
    IConsoleVariable* ForceRaw = bOverrideRaw ? IConsoleManager::Get().FindConsoleVariable(TEXT("a.ForceEvalRawData")) : nullptr;
    if (bOverrideRaw && ForceRaw == nullptr) { return nullptr; }
    const int32 PreviousRaw = bOverrideRaw ? ForceRaw->GetInt() : 0;
    const uint32 PreviousRawFlags = bOverrideRaw ? ForceRaw->GetFlags() : 0;
    const FName RawOverrideTag = bOverrideRaw ? FName(TEXT("AlsGraphSourceReaderRawProbe")) : NAME_None;
    TSharedPtr<FJsonObject> RawControl;
    ON_SCOPE_EXIT
    {
        if (bOverrideRaw)
        {
            ForceRaw->Unset(ECVF_SetByTemp, RawOverrideTag);
            RawControl->SetNumberField(TEXT("restored_value"), ForceRaw->GetInt());
            RawControl->SetNumberField(TEXT("restored_flags"), ForceRaw->GetFlags());
        }
    };
    if (bOverrideRaw)
    {
        RawControl = MakeShared<FJsonObject>();
        RawControl->SetBoolField(TEXT("requested"), bForceRaw);
        RawControl->SetStringField(TEXT("stage"), TEXT("before_world_creation"));
        RawControl->SetNumberField(TEXT("previous_value"), PreviousRaw);
        RawControl->SetNumberField(TEXT("previous_flags"), PreviousRawFlags);
        ForceRaw->Set(static_cast<int32>(bForceRaw), ECVF_SetByTemp, RawOverrideTag);
        RawControl->SetNumberField(TEXT("applied_value"), ForceRaw->GetInt());
        if (ForceRaw->GetInt() != static_cast<int32>(bForceRaw)) { return nullptr; }
    }
    FAssetCompilingManager::Get().FinishAllCompilation();
    TArray<TSharedPtr<FJsonValue>> Results;
    for (const TSharedPtr<FJsonValue>& CaseValue : *Cases)
    {
        const TSharedPtr<FJsonObject>* Case = nullptr;
        const TArray<TSharedPtr<FJsonValue>>* Frames = nullptr;
        FString Name;
        double Height;
        bool bDynamicBase;
        if (!CaseValue->TryGetObject(Case) || !(*Case)->TryGetStringField(TEXT("name"), Name) || Name.IsEmpty() ||
            !(*Case)->TryGetBoolField(TEXT("dynamic_base"), bDynamicBase) || !(*Case)->TryGetNumberField(TEXT("spawn_height_cm"), Height) || !FMath::IsFinite(Height) || Height <= 0 ||
            !(*Case)->TryGetArrayField(TEXT("frames"), Frames) || Frames->IsEmpty() || Frames->Num() > 240) { return nullptr; }
        bool bLocalController = false;
        if ((*Case)->HasField(TEXT("local_controller")) &&
            (!(*Case)->TryGetBoolField(TEXT("local_controller"), bLocalController) || (*Case)->TryGetField(TEXT("local_controller"))->Type != EJson::Boolean)) { return nullptr; }
        UWorld::InitializationValues Settings;
        Settings.AllowAudioPlayback(false).RequiresHitProxies(false).CreatePhysicsScene(true).ShouldSimulatePhysics(true).CreateNavigation(false).CreateAISystem(false).CreateFXSystem(false).SetTransactional(false);
        UWorld* World = UWorld::CreateWorld(EWorldType::Game, false, NAME_None, nullptr, true, ERHIFeatureLevel::Num, &Settings);
        if (World == nullptr || GEngine == nullptr) { return nullptr; }
        GEngine->CreateNewWorldContext(EWorldType::Game).SetCurrentWorld(World);
        World->GetWorldSettings()->GlobalGravityZ = static_cast<float>(Gravity);
        World->GetWorldSettings()->bGlobalGravitySet = true;
        World->GetWorldSettings()->bWorldGravitySet = false;
        const uint64 SavedFrameCounter = GFrameCounter;
        GFrameCounter = static_cast<uint64>(InitialEngineFrame);
        ON_SCOPE_EXIT
        {
            World->EndPlay(EEndPlayReason::Quit);
            GFrameCounter = SavedFrameCounter;
            GEngine->DestroyWorldContext(World);
            World->DestroyWorld(false);
        };
        AActor* Floor = World->SpawnActor<AActor>();
        if (Floor == nullptr) { return nullptr; }
        UBoxComponent* Box = NewObject<UBoxComponent>(Floor);
        Floor->SetRootComponent(Box);
        Floor->AddInstanceComponent(Box);
        Box->SetMobility(bDynamicBase ? EComponentMobility::Movable : EComponentMobility::Static);
        Box->SetBoxExtent(FVector(1500, 1500, 50));
        Box->SetWorldLocation(FVector(0, 0, -50));
        Box->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
        Box->SetCollisionResponseToAllChannels(ECR_Block);
        Box->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
        Box->RegisterComponent();
        FActorSpawnParameters Spawn;
        Spawn.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
        FObservedPose HeadObservation, RootObservation, HeadBaseObservation, HeadAdditiveObservation, BaseProviderInputObservation;
        FAnimNode_ControlRig* ObservedRigNode = nullptr;
        FAnimNode_Root* ObservedRootNode = nullptr;
        FAnimNode_ApplyMeshSpaceAdditive* ObservedHeadNode = nullptr;
        FAnimNode_LinkedAnimGraph* ObservedBaseProvider = nullptr;
        TWeakObjectPtr<UAnimInstance> ObservedInstance;
        TWeakObjectPtr<UAnimInstance> ObservedHeadInstance;
        TWeakObjectPtr<UAnimInstance> ObservedBaseProviderOwner;
        TWeakObjectPtr<UControlRig> ObservedRig;
        bool bInvalidObservationOwner = false;
        FDelegateHandle ConstructedHandle;
        ON_SCOPE_EXIT
        {
            if (ConstructedHandle.IsValid()) { FCoreUObjectDelegates::OnObjectConstructed.Remove(ConstructedHandle); }
            if (ObservedInstance.IsValid())
            {
                if (ObservedRigNode != nullptr) { FObservedRigAccess::SourceLink(*ObservedRigNode) = HeadObservation.Source; }
                if (ObservedRootNode != nullptr) { ObservedRootNode->Result = RootObservation.Source; }
            }
            if (ObservedBaseProviderOwner.IsValid() && ObservedBaseProvider != nullptr) { ObservedBaseProvider->InputPoses[0] = BaseProviderInputObservation.Source; }
            if (ObservedHeadInstance.IsValid() && ObservedHeadNode != nullptr)
            {
                ObservedHeadNode->Base = HeadBaseObservation.Source;
                ObservedHeadNode->Additive = HeadAdditiveObservation.Source;
            }
        };
        if (bCapturePoseBoundaries)
        {
            ConstructedHandle = FCoreUObjectDelegates::OnObjectConstructed.AddLambda([&](UObject* Object)
            {
                if (!IsInGameThread()) { return; }
                if (UAnimInstance* Main = Cast<UAnimInstance>(Object); Main != nullptr && Main->GetClass() == MainClass && Main->GetWorld() == World && !Main->HasAnyFlags(RF_ClassDefaultObject | RF_ArchetypeObject))
                {
                    if (ObservedBaseProvider != nullptr) { bInvalidObservationOwner = true; return; }
                    ObservedBaseProvider = BaseProviderProperty->ContainerPtrToValuePtr<FAnimNode_LinkedAnimGraph>(Main);
                    if (ObservedBaseProvider->InputPoses.IsEmpty()) { bInvalidObservationOwner = true; ObservedBaseProvider = nullptr; return; }
                    BaseProviderInputObservation.Source = ObservedBaseProvider->InputPoses[0];
                    ObservedBaseProvider->InputPoses[0].SetLinkNode(&BaseProviderInputObservation);
                    ObservedBaseProviderOwner = Main;
                }
                if (UAnimInstance* Linked = Cast<UAnimInstance>(Object); Linked != nullptr && Linked->GetClass() == HeadClass && Linked->GetWorld() == World && !Linked->HasAnyFlags(RF_ClassDefaultObject | RF_ArchetypeObject))
                {
                    if (ObservedHeadNode != nullptr) { bInvalidObservationOwner = true; return; }
                    ObservedHeadNode = HeadProperty->ContainerPtrToValuePtr<FAnimNode_ApplyMeshSpaceAdditive>(Linked);
                    HeadBaseObservation.Source = ObservedHeadNode->Base;
                    HeadAdditiveObservation.Source = ObservedHeadNode->Additive;
                    ObservedHeadNode->Base.SetLinkNode(&HeadBaseObservation);
                    ObservedHeadNode->Additive.SetLinkNode(&HeadAdditiveObservation);
                    ObservedHeadInstance = Linked;
                }
                UControlRig* NewRig = Cast<UControlRig>(Object);
                USkeletalMeshComponent* Component = NewRig != nullptr ? NewRig->GetTypedOuter<USkeletalMeshComponent>() : nullptr;
                UAnimInstance* Owner = Component != nullptr ? Component->GetAnimInstance() : nullptr;
                if (Owner == nullptr || Owner->HasAnyFlags(RF_ClassDefaultObject | RF_ArchetypeObject) || Owner->GetWorld() != World) { return; }
                if (ObservedRigNode != nullptr) { bInvalidObservationOwner = true; return; }
                const IAnimClassInterface* OwnerInterface = IAnimClassInterface::GetFromClass(Owner->GetClass());
                if (OwnerInterface == nullptr || OwnerInterface->GetAnimBlueprintFunctions().IsEmpty()) { bInvalidObservationOwner = true; return; }
                FStructProperty* RootProperty = CastField<FStructProperty>(OwnerInterface->GetAnimBlueprintFunctions()[0].OutputPoseNodeProperty);
                if (RootProperty == nullptr || RootProperty->Struct != FAnimNode_Root::StaticStruct()) { bInvalidObservationOwner = true; return; }
                FAnimNode_ControlRig* Input = nullptr;
                for (const FStructProperty* Property : OwnerInterface->GetAnimNodeProperties())
                {
                    if (Property->Struct != FAnimNode_ControlRig::StaticStruct()) { continue; }
                    if (Input != nullptr) { bInvalidObservationOwner = true; return; }
                    Input = Property->ContainerPtrToValuePtr<FAnimNode_ControlRig>(Owner);
                }
                FAnimNode_Root* Root = RootProperty->ContainerPtrToValuePtr<FAnimNode_Root>(Owner);
                if (Input == nullptr || FInstanceAccess::Proxy(Owner).GetRootNode() != Root) { bInvalidObservationOwner = true; return; }
                HeadObservation.Source = FObservedRigAccess::SourceLink(*Input);
                RootObservation.Source = Root->Result;
                FObservedRigAccess::SourceLink(*Input).SetLinkNode(&HeadObservation);
                Root->Result.SetLinkNode(&RootObservation);
                ObservedInstance = Owner;
                ObservedRig = NewRig;
                ObservedRigNode = Input;
                ObservedRootNode = Root;
            });
        }
        ACharacter* Character = World->SpawnActor<ACharacter>(Class, FVector(0, 0, Height), FRotator::ZeroRotator, Spawn);
        if (Character == nullptr) { return nullptr; }
        AGameModeBase* ControllerGameMode = nullptr;
        APlayerController* Controller = nullptr;
        if (bLocalController)
        {
            ControllerGameMode = World->SpawnActor<AGameModeBase>();
            if (ControllerGameMode == nullptr) { return nullptr; }
            Controller = ControllerGameMode->SpawnPlayerController(ROLE_SimulatedProxy, FString());
            if (Controller == nullptr) { return nullptr; }
        }
        World->InitializeActorsForPlay(FURL());
        if (bLocalController)
        {
            if (!Controller->IsActorInitialized() || Controller->PlayerCameraManager == nullptr)
            {
                UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Source controller actors are not initialized."));
                return nullptr;
            }
            Controller->Possess(Character);
            if (Controller->GetPawn() != Character || Character->GetController() != Controller || !Controller->IsLocalController())
            {
                UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Source controller setup failed: possessed=%d local=%d."), Controller->GetPawn() == Character, Controller->IsLocalController());
                return nullptr;
            }
        }
        World->BeginPlay();
        World->GetWorldSettings()->NotifyBeginPlay();
        USkeletalMeshComponent* Mesh = Character->GetMesh();
        UAnimInstance* Instance = Mesh != nullptr ? Mesh->GetAnimInstance() : nullptr;
        if (Instance == nullptr || Mesh->GetSkeletalMeshAsset() == nullptr) { return nullptr; }
        const FObjectPropertyBase* OwnerProperty = FindFProperty<FObjectPropertyBase>(Instance->GetClass(), TEXT("Character"));
        const FObjectPropertyBase* SettingsProperty = FindFProperty<FObjectPropertyBase>(Instance->GetClass(), TEXT("Settings"));
        if (OwnerProperty == nullptr || SettingsProperty == nullptr || OwnerProperty->GetObjectPropertyValue_InContainer(Instance) != Character ||
            !IsValid(SettingsProperty->GetObjectPropertyValue_InContainer(Instance))) { return nullptr; }
        const IAnimClassInterface* Interface = IAnimClassInterface::GetFromClass(Instance->GetClass());
        if (Interface == nullptr) { return nullptr; }
        FAnimNode_ControlRig* RigNode = nullptr;
        for (const FStructProperty* Property : Interface->GetAnimNodeProperties())
        {
            if (Property->Struct == FAnimNode_ControlRig::StaticStruct())
            {
                if (RigNode != nullptr) { return nullptr; }
                RigNode = Property->ContainerPtrToValuePtr<FAnimNode_ControlRig>(Instance);
            }
        }
        if (RigNode == nullptr || RigNode->GetControlRig() == nullptr) { return nullptr; }
        UControlRig* Rig = RigNode->GetControlRig();
        if (bCapturePoseBoundaries && (bInvalidObservationOwner || ObservedInstance.Get() != Instance || ObservedRig.Get() != Rig ||
            ObservedRigNode != RigNode || !HeadObservation.Latest.IsValid() || !RootObservation.Latest.IsValid() ||
            !ObservedHeadInstance.IsValid() || !HeadBaseObservation.Latest.IsValid() || !HeadAdditiveObservation.Latest.IsValid() || !BaseProviderInputObservation.Latest.IsValid()))
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Initial source pose boundaries are unavailable: invalid_owner=%d observed_instance=%s instance=%s rig_source_evaluations=%d root_evaluations=%d."), bInvalidObservationOwner, *ObjectPath(ObservedInstance.Get()), *ObjectPath(Instance), HeadObservation.Evaluations, RootObservation.Evaluations);
            return nullptr;
        }
        const UCharacterMovementComponent* Movement = Character->GetCharacterMovement();
        const FProperty* LocomotionProperty = Character->GetClass()->FindPropertyByName(TEXT("LocomotionMode"));
        if (Movement == nullptr || LocomotionProperty == nullptr) { return nullptr; }
        const auto CaptureMovement = [&]()
        {
            TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
            Value->SetNumberField(TEXT("movement_mode"), Movement->MovementMode.GetValue());
            Value->SetStringField(TEXT("movement_mode_name"), UEnum::GetValueAsString(Movement->MovementMode.GetValue()));
            Value->SetNumberField(TEXT("custom_movement_mode"), Movement->CustomMovementMode);
            FString Locomotion;
            LocomotionProperty->ExportTextItem_Direct(Locomotion, LocomotionProperty->ContainerPtrToValuePtr<void>(Character), nullptr, Character, PPF_None);
            Value->SetStringField(TEXT("character_locomotion_mode"), Locomotion);
            Value->SetBoolField(TEXT("registered"), Movement->IsRegistered());
            Value->SetBoolField(TEXT("begun_play"), Movement->HasBegunPlay());
            Value->SetBoolField(TEXT("tick_registered"), Movement->PrimaryComponentTick.IsTickFunctionRegistered());
            Value->SetBoolField(TEXT("tick_enabled"), Movement->IsComponentTickEnabled());
            Value->SetObjectField(TEXT("velocity_cm_s"), CaptureVector(Movement->Velocity));
            Value->SetObjectField(TEXT("last_update_location_cm"), CaptureVector(Movement->GetLastUpdateLocation()));
            Value->SetObjectField(TEXT("last_update_velocity_cm_s"), CaptureVector(Movement->GetLastUpdateVelocity()));
            Value->SetBoolField(TEXT("walkable_floor"), Movement->CurrentFloor.IsWalkableFloor());
            Value->SetNumberField(TEXT("floor_distance_cm"), Movement->CurrentFloor.FloorDist);
            return Value;
        };
        const auto ApplyDesiredTag = [&](const TCHAR* FunctionName, const FString& Tag)
        {
            UFunction* Function = Character->FindFunction(FName(FunctionName));
            if (Function == nullptr || Function->NumParms != 1) { return false; }
            const FStructProperty* Parameter = nullptr;
            for (TFieldIterator<FProperty> Property(Function); Property; ++Property)
            {
                if (Property->HasAnyPropertyFlags(CPF_Parm) && !Property->HasAnyPropertyFlags(CPF_ReturnParm))
                {
                    Parameter = CastField<FStructProperty>(*Property);
                }
            }
            if (Parameter == nullptr || Parameter->Struct->GetFName() != TEXT("GameplayTag")) { return false; }
            FStructOnScope Parameters(Function);
            const FString Text = FString::Printf(TEXT("(TagName=\"%s\")"), *Tag);
            if (Parameter->ImportText_Direct(*Text, Parameter->ContainerPtrToValuePtr<void>(Parameters.GetStructMemory()), Character, PPF_None) == nullptr) { return false; }
            Character->ProcessEvent(Function, Parameters.GetStructMemory());
            return true;
        };
        const auto CaptureController = [&]()
        {
            TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
            Value->SetBoolField(TEXT("requested"), bLocalController);
            Value->SetStringField(TEXT("controller_class"), Controller != nullptr ? ObjectPath(Controller->GetClass()) : FString());
            Value->SetBoolField(TEXT("possesses_source_character"), Controller != nullptr && Controller->GetPawn() == Character && Character->GetController() == Controller);
            Value->SetBoolField(TEXT("is_local_controller"), Controller != nullptr && Controller->IsLocalController());
            Value->SetBoolField(TEXT("character_is_locally_controlled"), Character->IsLocallyControlled());
            Value->SetBoolField(TEXT("has_authority"), Character->HasAuthority());
            Value->SetStringField(TEXT("game_mode_class"), ControllerGameMode != nullptr ? ObjectPath(ControllerGameMode->GetClass()) : FString());
            for (const TCHAR* Name : {TEXT("DesiredStance"), TEXT("DesiredGait")})
            {
                const FProperty* Property = Character->GetClass()->FindPropertyByName(FName(Name));
                if (Property == nullptr) { continue; }
                FString Text;
                Property->ExportTextItem_Direct(Text, Property->ContainerPtrToValuePtr<void>(Character), nullptr, Character, PPF_None);
                Value->SetStringField(Name, Text);
            }
            for (const TCHAR* Name : {TEXT("ViewState"), TEXT("HeadState")})
            {
                const FStructProperty* Property = FindFProperty<FStructProperty>(Instance->GetClass(), FName(Name));
                if (Property != nullptr) { Value->SetArrayField(Name, CaptureProperties(Property->Struct, Property->ContainerPtrToValuePtr<void>(Instance), Instance)); }
            }
            if (Controller != nullptr)
            {
                const FRotator Rotation = Controller->GetControlRotation();
                TSharedRef<FJsonObject> Control = MakeShared<FJsonObject>();
                Control->SetNumberField(TEXT("pitch"), Rotation.Pitch);
                Control->SetNumberField(TEXT("yaw"), Rotation.Yaw);
                Control->SetNumberField(TEXT("roll"), Rotation.Roll);
                Value->SetObjectField(TEXT("control_rotation_degrees"), Control);
            }
            return Value;
        };
        const auto CapturePoseBoundaries = [&]()
        {
            TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
            Value->SetStringField(TEXT("stage"), TEXT("actual_rig_source_and_main_root_result"));
            Value->SetObjectField(TEXT("rig_source"), HeadObservation.Latest.ToSharedRef());
            Value->SetObjectField(TEXT("root_result"), RootObservation.Latest.ToSharedRef());
            Value->SetObjectField(TEXT("head_base"), HeadBaseObservation.Latest.ToSharedRef());
            Value->SetObjectField(TEXT("head_additive"), HeadAdditiveObservation.Latest.ToSharedRef());
            Value->SetNumberField(TEXT("head_alpha"), ObservedHeadNode->ActualAlpha);
            Value->SetBoolField(TEXT("head_root_space_additive"), ObservedHeadNode->bRootSpaceAdditive);
            Value->SetStringField(TEXT("head_instance_class"), ObjectPath(ObservedHeadInstance->GetClass()));
            Value->SetStringField(TEXT("head_property"), HeadProperty->GetName());
            Value->SetObjectField(TEXT("base_provider_input"), BaseProviderInputObservation.Latest.ToSharedRef());
            Value->SetStringField(TEXT("base_provider_class"), ObjectPath(BaseProviderClass));
            return Value;
        };
        int32 Forward = 0;
        TArray<TSharedPtr<FJsonValue>> Executions;
        const FDelegateHandle ForwardHandle = Rig->OnPostForwardsSolve_AnyThread().AddLambda([&Forward](UControlRig*, const FName&) { ++Forward; });
        const FDelegateHandle ExecutionHandle = Rig->OnExecuted_AnyThread().AddLambda([&Executions](URigVMHost* Host, const FName& Event)
        {
            const FRigVMExtendedExecuteContext& Context = Host->GetRigVMExtendedExecuteContext();
            TSharedRef<FJsonObject> Execution = MakeShared<FJsonObject>();
            Execution->SetStringField(TEXT("entry"), Event.ToString());
            Execution->SetBoolField(TEXT("succeeded"), Context.CurrentExecuteResult == ERigVMExecuteResult::Succeeded);
            Executions.Add(MakeShared<FJsonValueObject>(Execution));
        });
        ON_SCOPE_EXIT
        {
            Rig->OnPostForwardsSolve_AnyThread().Remove(ForwardHandle);
            Rig->OnExecuted_AnyThread().Remove(ExecutionHandle);
        };
        const FAnimInstanceProxy& Proxy = FInstanceAccess::Proxy(Instance);
        const int32 InitialUpdate = Proxy.GetUpdateCounter().Get();
        const int32 InitialEvaluation = Proxy.GetEvaluationCounter().Get();
        TSharedRef<FJsonObject> Initialization = MakeShared<FJsonObject>();
        Initialization->SetStringField(TEXT("stage"), TEXT("after_begin_play_before_world_tick"));
        Initialization->SetNumberField(TEXT("world_seconds"), World->GetTimeSeconds());
        Initialization->SetNumberField(TEXT("update_counter"), InitialUpdate);
        Initialization->SetNumberField(TEXT("evaluation_counter"), InitialEvaluation);
        Initialization->SetObjectField(TEXT("actor_transform"), CaptureTransform(Character->GetActorTransform()));
        Initialization->SetObjectField(TEXT("actor_velocity_cm_s"), CaptureVector(Character->GetVelocity()));
        Initialization->SetObjectField(TEXT("movement_component"), CaptureMovement());
        if (bLocalController) { Initialization->SetObjectField(TEXT("controller_input"), CaptureController()); }
        Initialization->SetArrayField(TEXT("source_state"), CaptureState(Instance));
        Initialization->SetArrayField(TEXT("instances"), CaptureInstances(Mesh, Instance));
        const TSharedPtr<FJsonObject> InitialGetter = CaptureGetter(Instance);
        if (!InitialGetter.IsValid()) { return nullptr; }
        Initialization->SetObjectField(TEXT("getter"), InitialGetter.ToSharedRef());
        TArray<TSharedPtr<FJsonValue>> InitialPose;
        const TArray<FTransform> InitialTransforms = Mesh->GetBoneSpaceTransforms();
        const FReferenceSkeleton& InitialReference = Mesh->GetSkeletalMeshAsset()->GetRefSkeleton();
        if (InitialTransforms.Num() != InitialReference.GetNum()) { return nullptr; }
        for (int32 Index = 0; Index < InitialTransforms.Num(); ++Index)
        {
            TSharedRef<FJsonObject> Bone = MakeShared<FJsonObject>();
            Bone->SetStringField(TEXT("name"), InitialReference.GetBoneName(Index).ToString());
            Bone->SetObjectField(TEXT("local"), CaptureTransform(InitialTransforms[Index]));
            InitialPose.Add(MakeShared<FJsonValueObject>(Bone));
        }
        Initialization->SetArrayField(TEXT("mesh_local_pose"), InitialPose);
        TArray<FName> InitialCurveNames;
        Instance->GetActiveCurveNames(EAnimCurveType::AttributeCurve, InitialCurveNames);
        TSharedRef<FJsonObject> InitialCurves = MakeShared<FJsonObject>();
        for (FName Curve : InitialCurveNames) { InitialCurves->SetNumberField(Curve.ToString(), Instance->GetCurveValue(Curve)); }
        Initialization->SetObjectField(TEXT("curves"), InitialCurves);
        TArray<TSharedPtr<FJsonValue>> InitialRigPose;
        URigHierarchy* InitialHierarchy = Rig->GetHierarchy();
        if (InitialHierarchy == nullptr) { return nullptr; }
        for (const FRigElementKey& Key : InitialHierarchy->GetBoneKeys())
        {
            TSharedRef<FJsonObject> Bone = MakeShared<FJsonObject>();
            Bone->SetStringField(TEXT("name"), Key.Name.ToString());
            Bone->SetObjectField(TEXT("local"), CaptureTransform(InitialHierarchy->GetLocalTransform(Key)));
            InitialRigPose.Add(MakeShared<FJsonValueObject>(Bone));
        }
        Initialization->SetArrayField(TEXT("rig_local_pose"), InitialRigPose);
        TSharedRef<FJsonObject> InitialRigVariables = MakeShared<FJsonObject>();
        for (const FRigVMExternalVariable& Variable : Rig->GetExternalVariables())
        {
            TSharedRef<FJsonObject> Item = MakeShared<FJsonObject>();
            Item->SetStringField(TEXT("value"), Rig->GetVariableAsString(Variable.GetName()));
            if (const FStructProperty* Struct = CastField<FStructProperty>(Variable.GetProperty())) { Item->SetArrayField(TEXT("fields"), CaptureProperties(Struct->Struct, Variable.GetMemory(), Rig)); }
            InitialRigVariables->SetObjectField(Variable.GetName().ToString(), Item);
        }
        Initialization->SetObjectField(TEXT("rig_variables"), InitialRigVariables);
        if (bCapturePoseBoundaries) { Initialization->SetObjectField(TEXT("pose_boundaries"), CapturePoseBoundaries()); }
        TArray<TSharedPtr<FJsonValue>> Rows;
        for (const TSharedPtr<FJsonValue>& FrameValue : *Frames)
        {
            const TSharedPtr<FJsonObject>* Frame = nullptr;
            double Delta;
            bool bRendered;
            if (!FrameValue->TryGetObject(Frame) || !(*Frame)->TryGetNumberField(TEXT("delta_seconds"), Delta) || !FMath::IsFinite(static_cast<float>(Delta)) ||
                Delta <= 0 || Delta > 0.25 || !(*Frame)->TryGetBoolField(TEXT("rendered"), bRendered)) { return nullptr; }
            if ((*Frame)->HasField(TEXT("control_rotation_degrees")))
            {
                const TSharedPtr<FJsonObject>* Rotation = nullptr;
                double Pitch, Yaw, Roll;
                if (Controller == nullptr || !(*Frame)->TryGetObjectField(TEXT("control_rotation_degrees"), Rotation) ||
                    !(*Rotation)->HasTypedField<EJson::Number>(TEXT("pitch")) || !(*Rotation)->HasTypedField<EJson::Number>(TEXT("yaw")) || !(*Rotation)->HasTypedField<EJson::Number>(TEXT("roll")) ||
                    !(*Rotation)->TryGetNumberField(TEXT("pitch"), Pitch) || !(*Rotation)->TryGetNumberField(TEXT("yaw"), Yaw) || !(*Rotation)->TryGetNumberField(TEXT("roll"), Roll) ||
                    !FMath::IsFinite(Pitch) || !FMath::IsFinite(Yaw) || !FMath::IsFinite(Roll)) { return nullptr; }
                Controller->SetControlRotation(FRotator(Pitch, Yaw, Roll));
            }
            for (const auto& Desired : {TPair<const TCHAR*, const TCHAR*>(TEXT("desired_stance"), TEXT("SetDesiredStance")), TPair<const TCHAR*, const TCHAR*>(TEXT("desired_gait"), TEXT("SetDesiredGait"))})
            {
                if (!(*Frame)->HasField(Desired.Key)) { continue; }
                FString Tag;
                if (Controller == nullptr || !(*Frame)->TryGetStringField(Desired.Key, Tag)) { return nullptr; }
                const bool bValid = FCString::Strcmp(Desired.Key, TEXT("desired_stance")) == 0
                    ? (Tag == TEXT("Als.Stance.Standing") || Tag == TEXT("Als.Stance.Crouching"))
                    : (Tag == TEXT("Als.Gait.Walking") || Tag == TEXT("Als.Gait.Running") || Tag == TEXT("Als.Gait.Sprinting"));
                if (!bValid || !ApplyDesiredTag(Desired.Value, Tag)) { return nullptr; }
            }
            TSharedPtr<FJsonObject> ControllerBefore;
            if (bLocalController) { ControllerBefore = CaptureController(); }
            const bool bPoseTickedBeforeWorld = Mesh->PoseTickedThisFrame();
            const TSharedRef<FJsonObject> MovementBefore = CaptureMovement();
            const TSharedPtr<FJsonObject> GetterBefore = CaptureGetter(Instance);
            if (!GetterBefore.IsValid()) { return nullptr; }
            const double SavedDelta = FApp::GetDeltaTime();
            FApp::SetDeltaTime(Delta);
            ON_SCOPE_EXIT { FApp::SetDeltaTime(SavedDelta); };
            Mesh->SetLastRenderTime(bRendered ? World->GetTimeSeconds() : -1000.0f);
            World->Tick(LEVELTICK_All, static_cast<float>(Delta));
            ++GFrameCounter;
            Mesh->HandleExistingParallelEvaluationTask(true, true);
            if (Mesh->GetAnimInstance() != Instance || RigNode->GetControlRig() != Rig) { return nullptr; }
            TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
            Row->SetNumberField(TEXT("frame"), Rows.Num());
            Row->SetBoolField(TEXT("pose_ticked_before_world"), bPoseTickedBeforeWorld);
            Row->SetNumberField(TEXT("world_seconds"), World->GetTimeSeconds());
            Row->SetNumberField(TEXT("world_delta_seconds"), World->GetDeltaSeconds());
            Row->SetNumberField(TEXT("gravity_cm_s2"), World->GetGravityZ());
            Row->SetBoolField(TEXT("recently_rendered"), Mesh->bRecentlyRendered);
            Row->SetBoolField(TEXT("based_on_floor"), Character->GetMovementBase() == Box);
            Row->SetBoolField(TEXT("base_relative_location"), Character->GetBasedMovement().HasRelativeLocation());
            Row->SetBoolField(TEXT("actor_begun_play"), Character->HasActorBegunPlay());
            Row->SetNumberField(TEXT("update_counter"), Proxy.GetUpdateCounter().Get());
            Row->SetNumberField(TEXT("evaluation_counter"), Proxy.GetEvaluationCounter().Get());
            Row->SetNumberField(TEXT("rig_forward_events"), Forward);
            Row->SetObjectField(TEXT("actor_transform"), CaptureTransform(Character->GetActorTransform()));
            Row->SetObjectField(TEXT("actor_velocity_cm_s"), CaptureVector(Character->GetVelocity()));
            Row->SetObjectField(TEXT("mesh_to_world"), CaptureTransform(Mesh->GetComponentTransform()));
            Row->SetObjectField(TEXT("movement_before_tick"), MovementBefore);
            Row->SetObjectField(TEXT("movement_after_tick"), CaptureMovement());
            if (bLocalController) { Row->SetObjectField(TEXT("controller_before_tick"), ControllerBefore.ToSharedRef()); Row->SetObjectField(TEXT("controller_input"), CaptureController()); }
            Row->SetObjectField(TEXT("getter_before_tick"), GetterBefore.ToSharedRef());
            const TSharedPtr<FJsonObject> GetterAfter = CaptureGetter(Instance);
            if (!GetterAfter.IsValid()) { return nullptr; }
            Row->SetObjectField(TEXT("getter_after_tick"), GetterAfter.ToSharedRef());
            Row->SetArrayField(TEXT("source_state_after_tick"), CaptureState(Instance));
            TSharedRef<FJsonObject> Variables = MakeShared<FJsonObject>();
            for (const FRigVMExternalVariable& Variable : Rig->GetExternalVariables())
            {
                TSharedRef<FJsonObject> Item = MakeShared<FJsonObject>();
                Item->SetStringField(TEXT("value"), Rig->GetVariableAsString(Variable.GetName()));
                if (const FStructProperty* Struct = CastField<FStructProperty>(Variable.GetProperty())) { Item->SetArrayField(TEXT("fields"), CaptureProperties(Struct->Struct, Variable.GetMemory(), Rig)); }
                if (const FNumericProperty* Numeric = CastField<FNumericProperty>(Variable.GetProperty()); Numeric != nullptr && Numeric->IsFloatingPoint()) { Item->SetNumberField(TEXT("numeric_value"), Numeric->GetFloatingPointPropertyValue(Variable.GetMemory())); }
                Variables->SetObjectField(Variable.GetName().ToString(), Item);
            }
            Row->SetObjectField(TEXT("rig_variables_after_tick"), Variables);
            const TArray<FTransform> Transforms = Mesh->GetBoneSpaceTransforms();
            const FReferenceSkeleton& Reference = Mesh->GetSkeletalMeshAsset()->GetRefSkeleton();
            if (Transforms.Num() != Reference.GetNum()) { return nullptr; }
            TArray<TSharedPtr<FJsonValue>> Pose;
            int32 DifferentReferenceBones = 0;
            for (int32 Index = 0; Index < Transforms.Num(); ++Index)
            {
                if (Transforms[Index].ContainsNaN()) { return nullptr; }
                TSharedRef<FJsonObject> Bone = MakeShared<FJsonObject>();
                Bone->SetStringField(TEXT("name"), Reference.GetBoneName(Index).ToString());
                Bone->SetObjectField(TEXT("local"), CaptureTransform(Transforms[Index]));
                Pose.Add(MakeShared<FJsonValueObject>(Bone));
                if (!Transforms[Index].Equals(Reference.GetRefBonePose()[Index], 0.0)) { ++DifferentReferenceBones; }
            }
            Row->SetArrayField(TEXT("mesh_local_pose"), Pose);
            Row->SetNumberField(TEXT("different_reference_bones"), DifferentReferenceBones);
            TArray<FName> CurveNames;
            Instance->GetActiveCurveNames(EAnimCurveType::AttributeCurve, CurveNames);
            TSharedRef<FJsonObject> Curves = MakeShared<FJsonObject>();
            for (FName Curve : CurveNames) { Curves->SetNumberField(Curve.ToString(), Instance->GetCurveValue(Curve)); }
            Row->SetObjectField(TEXT("curves"), Curves);
            Row->SetArrayField(TEXT("linked_instances"), CaptureInstances(Mesh, Instance));
            Row->SetArrayField(TEXT("rig_executions"), Executions);
            if (bCapturePoseBoundaries)
            {
                if (!HeadObservation.Latest.IsValid() || !RootObservation.Latest.IsValid() || !HeadBaseObservation.Latest.IsValid() || !HeadAdditiveObservation.Latest.IsValid()) { return nullptr; }
                Row->SetObjectField(TEXT("pose_boundaries"), CapturePoseBoundaries());
            }
            Executions.Reset();
            Rows.Add(MakeShared<FJsonValueObject>(Row));
            UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Source character animation %s frame %d/%d captured."), *Name, Rows.Num(), Frames->Num());
        }
        TSharedRef<FJsonObject> CaseRow = MakeShared<FJsonObject>();
        CaseRow->SetStringField(TEXT("name"), Name);
        CaseRow->SetNumberField(TEXT("initial_engine_frame"), InitialEngineFrame);
        CaseRow->SetBoolField(TEXT("floor_dynamic"), Box->GetMobility() == EComponentMobility::Movable);
        CaseRow->SetStringField(TEXT("character_class"), Character->GetClass()->GetPathName());
        CaseRow->SetStringField(TEXT("animation_class"), Instance->GetClass()->GetPathName());
        CaseRow->SetStringField(TEXT("settings"), SettingsProperty->GetObjectPropertyValue_InContainer(Instance)->GetPathName());
        CaseRow->SetBoolField(TEXT("source_character_bound"), OwnerProperty->GetObjectPropertyValue_InContainer(Instance) == Character);
        bool bTickAfterCharacter = false;
        for (const FTickPrerequisite& Prerequisite : Mesh->PrimaryComponentTick.GetPrerequisites()) { bTickAfterCharacter |= Prerequisite.PrerequisiteObject.Get() == Character; }
        CaseRow->SetBoolField(TEXT("mesh_tick_after_character"), bTickAfterCharacter);
        CaseRow->SetObjectField(TEXT("initialization"), Initialization);
        CaseRow->SetNumberField(TEXT("initial_update_counter"), InitialUpdate);
        CaseRow->SetNumberField(TEXT("initial_evaluation_counter"), InitialEvaluation);
        CaseRow->SetArrayField(TEXT("frames"), Rows);
        Results.Add(MakeShared<FJsonValueObject>(CaseRow));
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetArrayField(TEXT("cases"), Results);
    if (bOverrideRaw)
    {
        RawControl->SetNumberField(TEXT("value_after_worlds"), ForceRaw->GetInt());
        Result->SetObjectField(TEXT("raw_data_control"), RawControl);
    }
    return Result;
}

TSharedPtr<FJsonObject> CaptureRigNodeHistoryProbe(UObject* Asset, const FString& RequestPath)
{
    FMemMark Mark(FMemStack::Get());
    UBlueprint* Blueprint = Cast<UBlueprint>(Asset);
    UAnimBlueprintGeneratedClass* Class = Blueprint != nullptr ? Cast<UAnimBlueprintGeneratedClass>(Blueprint->GeneratedClass) : nullptr;
    const IAnimClassInterface* Interface = Class != nullptr ? IAnimClassInterface::GetFromClass(Class) : nullptr;
    FString Text, GuidTextValue, MeshPath;
    FGuid Guid;
    TSharedPtr<FJsonObject> Request;
    const TArray<TSharedPtr<FJsonValue>>* Names = nullptr;
    const TArray<TSharedPtr<FJsonValue>>* Cases = nullptr;
    if (Interface == nullptr || !FFileHelper::LoadFileToString(Text, *RequestPath) || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) ||
        !Request->TryGetStringField(TEXT("node_guid"), GuidTextValue) || !FGuid::Parse(GuidTextValue, Guid) || !Guid.IsValid() ||
        !Request->TryGetStringField(TEXT("mesh_asset"), MeshPath) || !Request->TryGetArrayField(TEXT("bone_names"), Names) ||
        !Request->TryGetArrayField(TEXT("cases"), Cases) || Cases->IsEmpty() || Cases->Num() > 32) { return nullptr; }
    USkeletalMesh* MeshAsset = LoadObject<USkeletalMesh>(nullptr, *MeshPath);
    if (MeshAsset == nullptr || MeshAsset->GetSkeleton() != Interface->GetTargetSkeleton()) { return nullptr; }
    const FReferenceSkeleton& Reference = MeshAsset->GetRefSkeleton();
    if (Names->Num() != Reference.GetNum()) { return nullptr; }
    TArray<FBoneIndexType> Bones;
    for (int32 Index = 0; Index < Names->Num(); ++Index)
    {
        FString Name;
        if (!(*Names)[Index]->TryGetString(Name) || Name != Reference.GetBoneName(Index).ToString()) { return nullptr; }
        Bones.Add(Index);
    }
    const int32* NodeIndex = Class->GetNodePropertyIndexFromGuid(Guid);
    const TArray<FStructProperty*>& Properties = Interface->GetAnimNodeProperties();
    const int32 PropertyIndex = NodeIndex != nullptr ? Properties.Num() - 1 - *NodeIndex : INDEX_NONE;
    if (!Properties.IsValidIndex(PropertyIndex) || Properties[PropertyIndex]->Struct != FAnimNode_ControlRig::StaticStruct()) { return nullptr; }
    const UEdGraphNode* Visual = Class->GetVisualNodeFromNodePropertyIndex(PropertyIndex);
    if (Visual == nullptr || Visual->NodeGuid != Guid) { return nullptr; }
    struct FNodeAccess : FAnimNode_ControlRig
    {
        static void SetSource(FAnimNode_ControlRig& Node, FAnimNode_Base* Leaf) { (Node.*&FNodeAccess::Source).SetLinkNode(Leaf); }
        static bool AdapterReady(const FAnimNode_ControlRig& Node) { return (Node.*&FNodeAccess::ControlRigHierarchyMappings).CheckPoseAdapter(); }
        static bool AdapterEnabled(const FAnimNode_ControlRig& Node) { return (Node.*&FNodeAccess::ControlRigHierarchyMappings).IsPoseAdapterEnabled(); }
    };
    struct FProbeProxy : FAnimInstanceProxy { using FAnimInstanceProxy::FAnimInstanceProxy; using FAnimInstanceProxy::Initialize; };
    const auto ReadVector = [](const TSharedPtr<FJsonObject>& Object, const TCHAR* Key, FVector& Vector)
    {
        const TArray<TSharedPtr<FJsonValue>>* Array = nullptr;
        double Values[3];
        if (!Object->TryGetArrayField(Key, Array) || Array->Num() != 3) { return false; }
        for (int32 Index = 0; Index < 3; ++Index) { if ((*Array)[Index]->Type != EJson::Number || !(*Array)[Index]->TryGetNumber(Values[Index]) || !FMath::IsFinite(Values[Index])) { return false; } }
        Vector = FVector(Values[0], Values[1], Values[2]);
        return true;
    };
    TArray<TSharedPtr<FJsonValue>> CaseRows;
    for (const TSharedPtr<FJsonValue>& CaseValue : *Cases)
    {
        const TSharedPtr<FJsonObject>* Case = nullptr;
        const TArray<TSharedPtr<FJsonValue>>* Frames = nullptr;
        FString CaseName;
        FVector HalfExtents, BoxLocation;
        if (!CaseValue->TryGetObject(Case) || !(*Case)->TryGetStringField(TEXT("name"), CaseName) || CaseName.IsEmpty() ||
            !(*Case)->TryGetArrayField(TEXT("frames"), Frames) || Frames->IsEmpty() || Frames->Num() > 600 ||
            !ReadVector(*Case, TEXT("box_half_extents_cm"), HalfExtents) || HalfExtents.GetMin() <= 0.0 ||
            !ReadVector(*Case, TEXT("box_location_cm"), BoxLocation)) { return nullptr; }
        UWorld::InitializationValues Settings;
        Settings.AllowAudioPlayback(false).RequiresHitProxies(false).CreatePhysicsScene(true).ShouldSimulatePhysics(false).CreateNavigation(false).CreateAISystem(false).CreateFXSystem(false).SetTransactional(false);
        UWorld* World = UWorld::CreateWorld(EWorldType::Game, false, NAME_None, nullptr, true, ERHIFeatureLevel::Num, &Settings);
        if (World == nullptr) { return nullptr; }
        ON_SCOPE_EXIT { World->DestroyWorld(false); };
        AActor* Floor = World->SpawnActor<AActor>();
        AActor* Owner = World->SpawnActor<AActor>();
        if (Floor == nullptr || Owner == nullptr) { return nullptr; }
        UBoxComponent* Box = NewObject<UBoxComponent>(Floor);
        Floor->SetRootComponent(Box);
        Floor->AddInstanceComponent(Box);
        Box->SetBoxExtent(HalfExtents);
        Box->SetWorldLocation(BoxLocation);
        Box->SetCollisionEnabled(ECollisionEnabled::QueryOnly);
        Box->SetCollisionResponseToAllChannels(ECR_Block);
        Box->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
        Box->RegisterComponent();
        USkeletalMeshComponent* Mesh = NewObject<USkeletalMeshComponent>(Owner);
        Owner->SetRootComponent(Mesh);
        Owner->AddInstanceComponent(Mesh);
        Mesh->SetSkeletalMeshAsset(MeshAsset);
        Mesh->RegisterComponent();
        UAnimInstance* Instance = NewObject<UAnimInstance>(Mesh, Class);
        FProbeProxy Proxy(Instance);
        Proxy.Initialize(Instance);
        Proxy.GetRequiredBones().InitializeTo(Bones, UE::Anim::FCurveFilterSettings(), *MeshAsset);
        Instance->GetRequiredBones().InitializeTo(Bones, UE::Anim::FCurveFilterSettings(), *MeshAsset);
        FAnimNode_ControlRig& Node = *Properties[PropertyIndex]->ContainerPtrToValuePtr<FAnimNode_ControlRig>(Instance);
        FSourcePoseProbeLeaf Leaf;
        FNodeAccess::SetSource(Node, &Leaf);
        Node.OnInitializeAnimInstance(&Proxy, Instance);
        UControlRig* Rig = Node.GetControlRig();
        if (Rig == nullptr) { return nullptr; }
        TArray<TSharedPtr<FJsonValue>> Executions;
        int32 Constructed = 0, Forward = 0;
        bool bCaptureWorld = false;
        Rig->OnExecuted_AnyThread().AddLambda([&Executions, &bCaptureWorld](URigVMHost* Host, const FName& Event)
        {
            const FRigVMExtendedExecuteContext& Context = Host->GetRigVMExtendedExecuteContext();
            TSharedRef<FJsonObject> Execution = MakeShared<FJsonObject>();
            Execution->SetStringField(TEXT("entry"), Event.ToString());
            Execution->SetBoolField(TEXT("succeeded"), Context.CurrentExecuteResult == ERigVMExecuteResult::Succeeded);
            if (bCaptureWorld) { Execution->SetObjectField(TEXT("rig_to_world"), CaptureTransform(Context.GetPublicData<>().GetToWorldSpaceTransform())); }
            TArray<TSharedPtr<FJsonValue>> Visited;
#if WITH_EDITOR
            if (const FRigVMInstructionVisitInfo* Info = Context.GetRigVMInstructionVisitInfo()) { for (int32 Instruction : Info->GetInstructionVisitOrder()) { Visited.Add(MakeShared<FJsonValueNumber>(Instruction)); } }
#endif
            Execution->SetArrayField(TEXT("instructions"), Visited);
            Executions.Add(MakeShared<FJsonValueObject>(Execution));
        });
        Rig->OnPostConstruction_AnyThread().AddLambda([&Constructed](UControlRig*, const FName&) { ++Constructed; });
        Rig->OnPostForwardsSolve_AnyThread().AddLambda([&Forward](UControlRig*, const FName&) { ++Forward; });
        ON_SCOPE_EXIT
        {
            Rig->OnExecuted_AnyThread().Clear();
            Rig->OnPostConstruction_AnyThread().Clear();
            Rig->OnPostForwardsSolve_AnyThread().Clear();
        };
        Node.Initialize_AnyThread(FAnimationInitializeContext(&Proxy));
        Node.CacheBones_AnyThread(FAnimationCacheBonesContext(&Proxy));
        if (!FNodeAccess::AdapterReady(Node) || !FNodeAccess::AdapterEnabled(Node)) { return nullptr; }
        TArray<TSharedPtr<FJsonValue>> Rows;
        for (const TSharedPtr<FJsonValue>& FrameValue : *Frames)
        {
            const TSharedPtr<FJsonObject>* Frame = nullptr;
            const TSharedPtr<FJsonObject>* Variables = nullptr;
            double Delta;
            FString Name;
            FVector Start, End;
            if (!FrameValue->TryGetObject(Frame) || !(*Frame)->TryGetStringField(TEXT("name"), Name) || Name.IsEmpty() ||
                !(*Frame)->TryGetNumberField(TEXT("delta_seconds"), Delta) || !FMath::IsFinite(static_cast<float>(Delta)) || Delta < 0.0 || Delta > 0.25 ||
                !(*Frame)->TryGetObjectField(TEXT("variables"), Variables) || !(*Frame)->HasField(TEXT("pose")) ||
                !ReadVector(*Frame, TEXT("reference_ray_start_cm"), Start) || !ReadVector(*Frame, TEXT("reference_ray_end_cm"), End)) { return nullptr; }
            bCaptureWorld = (*Frame)->HasField(TEXT("mesh_to_world"));
            if (bCaptureWorld)
            {
                const TSharedPtr<FJsonObject>* Transform = nullptr;
                const TArray<TSharedPtr<FJsonValue>>* RotationValues = nullptr;
                FVector Translation, Scale;
                if (!(*Frame)->TryGetObjectField(TEXT("mesh_to_world"), Transform) || !ReadVector(*Transform, TEXT("translation_cm"), Translation) || !ReadVector(*Transform, TEXT("scale"), Scale) ||
                    !(*Transform)->TryGetArrayField(TEXT("rotation"), RotationValues) || RotationValues->Num() != 4) { return nullptr; }
                double Components[4];
                for (int32 Index = 0; Index < 4; ++Index)
                {
                    if ((*RotationValues)[Index]->Type != EJson::Number || !(*RotationValues)[Index]->TryGetNumber(Components[Index]) || !FMath::IsFinite(Components[Index])) { return nullptr; }
                }
                const FQuat Rotation(Components[0], Components[1], Components[2], Components[3]);
                if (!Rotation.IsNormalized()) { return nullptr; }
                const FTransform MeshToWorld(Rotation, Translation, Scale);
                Mesh->SetComponentToWorld(MeshToWorld);
            }
            Leaf.Transforms.Reset();
            Leaf.Curves.Reset();
            if (!ReadSourcePoseProbeLeaf(Reference, (*Frame)->TryGetField(TEXT("pose")), Leaf)) { return nullptr; }
            Node.Update_AnyThread(FAnimationUpdateContext(&Proxy, static_cast<float>(Delta)));
            for (const auto& Variable : (*Variables)->Values)
            {
                FString Value;
                if (!Variable.Value->TryGetString(Value) || !Rig->SetVariableFromString(FName(*Variable.Key), Value)) { return nullptr; }
            }
            TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
            Row->SetStringField(TEXT("name"), Name);
            Row->SetNumberField(TEXT("rig_id"), Node.GetControlRig()->GetUniqueID());
            Row->SetNumberField(TEXT("delta_seconds_after_update"), Rig->GetDeltaTime());
            Row->SetNumberField(TEXT("leaf_update_weight"), Leaf.UpdateWeight);
            if (bCaptureWorld) { Row->SetObjectField(TEXT("mesh_to_world"), CaptureTransform(Mesh->GetComponentTransform())); }
            FPoseContext Before(&Proxy), After(&Proxy);
            Leaf.Evaluate_AnyThread(Before);
            Row->SetObjectField(TEXT("before"), CaptureCompactPose(Before.Pose, Before.Curve));
            Node.Evaluate_AnyThread(After);
            Row->SetObjectField(TEXT("after"), CaptureCompactPose(After.Pose, After.Curve));
            Row->SetNumberField(TEXT("attribute_count"), After.CustomAttributes.Num());
            Row->SetNumberField(TEXT("forward_events"), Forward);
            Row->SetNumberField(TEXT("construction_events"), Constructed);
            FHitResult Hit;
            const bool bHit = World->LineTraceSingleByChannel(Hit, Start, End, ECC_Visibility, FCollisionQueryParams(NAME_None, true, Owner));
            TSharedRef<FJsonObject> Query = MakeShared<FJsonObject>();
            Query->SetBoolField(TEXT("blocking_hit"), bHit && Hit.bBlockingHit);
            Query->SetObjectField(TEXT("impact_point_cm"), CaptureVector(Hit.ImpactPoint));
            Query->SetObjectField(TEXT("impact_normal"), CaptureVector(Hit.ImpactNormal));
            Row->SetObjectField(TEXT("reference_query_after_forward"), Query);
            TSharedRef<FJsonObject> VariablesAfter = MakeShared<FJsonObject>();
            for (const FRigVMExternalVariable& Variable : Rig->GetExternalVariables())
            {
                TSharedRef<FJsonObject> VariableRow = MakeShared<FJsonObject>();
                VariableRow->SetStringField(TEXT("value"), Rig->GetVariableAsString(Variable.GetName()));
                if (const FStructProperty* Struct = CastField<FStructProperty>(Variable.GetProperty())) { VariableRow->SetArrayField(TEXT("fields"), CaptureProperties(Struct->Struct, Variable.GetMemory(), Rig)); }
                else if (const FNumericProperty* Numeric = CastField<FNumericProperty>(Variable.GetProperty()); Numeric != nullptr && Numeric->IsFloatingPoint()) { VariableRow->SetNumberField(TEXT("numeric_value"), Numeric->GetFloatingPointPropertyValue(Variable.GetMemory())); }
                VariablesAfter->SetObjectField(Variable.GetName().ToString(), VariableRow);
            }
            Row->SetObjectField(TEXT("externals_after"), VariablesAfter);
            const FRigVMMemoryStorageStruct* Work = Rig->GetVM()->GetWorkMemory(Rig->GetRigVMExtendedExecuteContext());
            if (Work == nullptr) { return nullptr; }
            Row->SetObjectField(TEXT("work_after"), CaptureRigMemory(*Work, Rig));
            Row->SetArrayField(TEXT("executions"), Executions);
            Executions.Reset();
            Rows.Add(MakeShared<FJsonValueObject>(Row));
            UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Rig node history %s frame %d/%d captured."), *CaseName, Rows.Num(), Frames->Num());
        }
        TSharedRef<FJsonObject> CaseRow = MakeShared<FJsonObject>();
        CaseRow->SetStringField(TEXT("name"), CaseName);
        CaseRow->SetBoolField(TEXT("game_world"), Rig->GetWorld() == World && World->IsGameWorld());
        CaseRow->SetBoolField(TEXT("bound_component"), Rig->GetObjectBinding().IsValid() && Rig->GetObjectBinding()->GetBoundObject() == Mesh);
        CaseRow->SetBoolField(TEXT("adapter_enabled"), FNodeAccess::AdapterEnabled(Node));
        CaseRow->SetBoolField(TEXT("adapter_ready"), FNodeAccess::AdapterReady(Node));
        CaseRow->SetObjectField(TEXT("mesh_to_world"), CaptureTransform(Mesh->GetComponentTransform()));
        CaseRow->SetStringField(TEXT("external_controls_stage"), TEXT("after_update_before_evaluate"));
        CaseRow->SetArrayField(TEXT("frames"), Rows);
        CaseRows.Add(MakeShared<FJsonValueObject>(CaseRow));
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetStringField(TEXT("node_property"), Properties[PropertyIndex]->GetName());
    Result->SetArrayField(TEXT("cases"), CaseRows);
    return Result;
}

TSharedPtr<FJsonObject> CaptureRigLifecycleProbe(UObject* Asset, const FString& RequestPath)
{
    struct FConstructionProbe : FAnimNode_ControlRigBase
    {
        UControlRig* Rig = nullptr;
        virtual UControlRig* GetControlRig() const override { return Rig; }
        virtual TSubclassOf<UControlRig> GetControlRigClass() const override { return Rig->GetClass(); }
        virtual FControlRigAssetStrongReference GetControlRigAssetReference() const override { return Rig->GetAssetReference(); }
        using FAnimNode_ControlRigBase::ExecuteConstructionIfNeeded;
    };
    const auto CaptureCurrent = [](const URigHierarchy* Hierarchy)
    {
        TSharedRef<FJsonObject> Pose = MakeShared<FJsonObject>();
        TArray<TSharedPtr<FJsonValue>> Bones;
        for (const FRigElementKey& Key : Hierarchy->GetBoneKeys())
        {
            TSharedRef<FJsonObject> Bone = MakeShared<FJsonObject>();
            Bone->SetStringField(TEXT("name"), Key.Name.ToString());
            Bone->SetObjectField(TEXT("local"), CaptureTransform(Hierarchy->GetLocalTransform(Key)));
            Bone->SetObjectField(TEXT("global"), CaptureTransform(Hierarchy->GetGlobalTransform(Key)));
            Bones.Add(MakeShared<FJsonValueObject>(Bone));
        }
        Pose->SetArrayField(TEXT("bones"), Bones);
        return Pose;
    };
    UBlueprint* Blueprint = Cast<UBlueprint>(Asset);
    UClass* Class = Blueprint != nullptr ? Blueprint->GeneratedClass : nullptr;
    FString Text;
    TSharedPtr<FJsonObject> Request;
    const TArray<TSharedPtr<FJsonValue>>* Samples = nullptr;
    if (Class == nullptr || !Class->IsChildOf(UControlRig::StaticClass()) ||
        !FFileHelper::LoadFileToString(Text, *RequestPath) || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) ||
        !Request->TryGetArrayField(TEXT("samples"), Samples) || Samples->IsEmpty() || Samples->Num() > 32) { return nullptr; }
    TArray<TSharedPtr<FJsonValue>> Results;
    for (const TSharedPtr<FJsonValue>& SampleValue : *Samples)
    {
        const TSharedPtr<FJsonObject>* Sample = nullptr;
        const TSharedPtr<FJsonObject>* Variables = nullptr;
        FString Name;
        bool bGameWorld = false;
        double Count = 0, Delta = 0;
        if (!SampleValue->TryGetObject(Sample) || !(*Sample)->TryGetStringField(TEXT("name"), Name) || Name.IsEmpty() ||
            !(*Sample)->TryGetBoolField(TEXT("game_world"), bGameWorld) || !(*Sample)->TryGetNumberField(TEXT("forward_count"), Count) ||
            !FMath::IsFinite(Count) || Count < 0 || Count > 240 || Count != FMath::FloorToDouble(Count) ||
            !(*Sample)->TryGetNumberField(TEXT("delta_seconds"), Delta) || !FMath::IsFinite(static_cast<float>(Delta)) || Delta < 0 || Delta > 0.25 ||
            !(*Sample)->TryGetObjectField(TEXT("variables"), Variables)) { return nullptr; }
        UWorld::InitializationValues Settings;
        Settings.AllowAudioPlayback(false).RequiresHitProxies(false).CreatePhysicsScene(true).ShouldSimulatePhysics(false).CreateNavigation(false).CreateAISystem(false).CreateFXSystem(false).SetTransactional(false);
        UWorld* World = UWorld::CreateWorld(bGameWorld ? EWorldType::Game : EWorldType::EditorPreview, false, NAME_None, nullptr, true, ERHIFeatureLevel::Num, &Settings);
        if (World == nullptr) { return nullptr; }
        ON_SCOPE_EXIT { World->DestroyWorld(false); };
        AActor* Owner = World->SpawnActor<AActor>();
        if (Owner == nullptr) { return nullptr; }
        USkeletalMeshComponent* Mesh = NewObject<USkeletalMeshComponent>(Owner);
        Owner->AddInstanceComponent(Mesh);
        Mesh->RegisterComponent();
        UControlRig* Rig = NewObject<UControlRig>(Mesh, Class);
        Rig->SetObjectBinding(MakeShared<FControlRigObjectBinding>());
        Rig->GetObjectBinding()->BindToObject(Mesh);
        int32 Initialized = 0, Constructed = 0, Forward = 0;
        TArray<TSharedPtr<FJsonValue>> Executions;
        Rig->OnExecuted_AnyThread().AddLambda([&Executions](URigVMHost* Host, const FName& Event)
        {
            const FRigVMExtendedExecuteContext& Context = Host->GetRigVMExtendedExecuteContext();
            TSharedRef<FJsonObject> Execution = MakeShared<FJsonObject>();
            Execution->SetStringField(TEXT("entry"), Event.ToString());
            Execution->SetBoolField(TEXT("succeeded"), Context.CurrentExecuteResult == ERigVMExecuteResult::Succeeded);
            TArray<TSharedPtr<FJsonValue>> Visited;
#if WITH_EDITOR
            if (const FRigVMInstructionVisitInfo* Info = Context.GetRigVMInstructionVisitInfo())
            {
                for (int32 Instruction : Info->GetInstructionVisitOrder()) { Visited.Add(MakeShared<FJsonValueNumber>(Instruction)); }
            }
#endif
            Execution->SetArrayField(TEXT("instructions"), Visited);
            Executions.Add(MakeShared<FJsonValueObject>(Execution));
        });
        Rig->OnInitialized_AnyThread().AddLambda([&Initialized](URigVMHost*, const FName&) { ++Initialized; });
        Rig->OnPostConstruction_AnyThread().AddLambda([&Constructed](UControlRig*, const FName&) { ++Constructed; });
        Rig->OnPostForwardsSolve_AnyThread().AddLambda([&Forward](UControlRig*, const FName&) { ++Forward; });
        ON_SCOPE_EXIT
        {
            Rig->OnInitialized_AnyThread().Clear();
            Rig->OnPostConstruction_AnyThread().Clear();
            Rig->OnPostForwardsSolve_AnyThread().Clear();
            Rig->OnExecuted_AnyThread().Clear();
        };
        Rig->Initialize(true);
        Rig->RequestInit();
        const int32 InitializedAfterSetup = Initialized;
        const int32 ConstructedAfterSetup = Constructed;
        for (const auto& Variable : (*Variables)->Values)
        {
            FString Value;
            if (!Variable.Value->TryGetString(Value) || !Rig->SetVariableFromString(FName(*Variable.Key), Value)) { return nullptr; }
        }
        FConstructionProbe Construction;
        Construction.Rig = Rig;
        Construction.ExecuteConstructionIfNeeded();
        TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
        Row->SetStringField(TEXT("name"), Name);
        Row->SetBoolField(TEXT("game_world"), Rig->GetWorld() != nullptr && Rig->GetWorld()->IsGameWorld());
        Row->SetNumberField(TEXT("initialization_events_after_setup"), InitializedAfterSetup);
        Row->SetNumberField(TEXT("construction_events_after_setup"), ConstructedAfterSetup);
        Row->SetNumberField(TEXT("construction_events_before_forward"), Constructed);
        Row->SetObjectField(TEXT("construction_pose"), CaptureCurrent(Rig->GetHierarchy()));
        TSharedRef<FJsonObject> ConstructionVariables = MakeShared<FJsonObject>();
        for (const FRigVMExternalVariable& Variable : Rig->GetExternalVariables()) { ConstructionVariables->SetStringField(Variable.GetName().ToString(), Rig->GetVariableAsString(Variable.GetName())); }
        Row->SetObjectField(TEXT("construction_variables"), ConstructionVariables);
        for (int32 Index = 0; Index < static_cast<int32>(Count); ++Index)
        {
            Rig->SetDeltaTime(static_cast<float>(Delta));
            if (!Rig->ExecuteEvent(FName(TEXT("Forwards Solve")))) { return nullptr; }
            UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Rig lifecycle sample %d forward %d/%d."), Results.Num(), Index + 1, static_cast<int32>(Count));
        }
        Row->SetNumberField(TEXT("initialization_events"), Initialized);
        Row->SetNumberField(TEXT("construction_events"), Constructed);
        Row->SetNumberField(TEXT("forward_events"), Forward);
        Row->SetArrayField(TEXT("executions"), Executions);
        Row->SetObjectField(TEXT("final_pose"), CaptureCurrent(Rig->GetHierarchy()));
        TSharedRef<FJsonObject> FinalVariables = MakeShared<FJsonObject>();
        for (const FRigVMExternalVariable& Variable : Rig->GetExternalVariables()) { FinalVariables->SetStringField(Variable.GetName().ToString(), Rig->GetVariableAsString(Variable.GetName())); }
        Row->SetObjectField(TEXT("final_variables"), FinalVariables);
        Results.Add(MakeShared<FJsonValueObject>(Row));
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetArrayField(TEXT("samples"), Results);
    return Result;
}

TSharedPtr<FJsonObject> CaptureMeshAdditiveProbe(UObject* Asset, const FString& RequestPath)
{
    FMemMark Mark(FMemStack::Get());
    UBlueprint* Blueprint = Cast<UBlueprint>(Asset);
    UAnimBlueprintGeneratedClass* Class = Blueprint != nullptr ? Cast<UAnimBlueprintGeneratedClass>(Blueprint->GeneratedClass) : nullptr;
    const IAnimClassInterface* Interface = Class != nullptr ? IAnimClassInterface::GetFromClass(Class) : nullptr;
    FString Text, NodeGuidText;
    TSharedPtr<FJsonObject> Request;
    const TArray<TSharedPtr<FJsonValue>>* Names = nullptr;
    const TArray<TSharedPtr<FJsonValue>>* Samples = nullptr;
    FGuid NodeGuid;
    if (Interface == nullptr || Interface->GetTargetSkeleton() == nullptr || !FFileHelper::LoadFileToString(Text, *RequestPath) ||
        !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) ||
        !Request->TryGetStringField(TEXT("node_guid"), NodeGuidText) || !FGuid::Parse(NodeGuidText, NodeGuid) || !NodeGuid.IsValid() ||
        !Request->TryGetArrayField(TEXT("bone_names"), Names) || !Request->TryGetArrayField(TEXT("samples"), Samples) ||
        Samples->IsEmpty() || Samples->Num() > 256) { return nullptr; }
    const int32* NodeIndex = Class->GetNodePropertyIndexFromGuid(NodeGuid);
    const TArray<FStructProperty*>& Properties = Interface->GetAnimNodeProperties();
    const int32 PropertyIndex = NodeIndex != nullptr ? Properties.Num() - 1 - *NodeIndex : INDEX_NONE;
    if (!Properties.IsValidIndex(PropertyIndex) || Properties[PropertyIndex]->Struct != FAnimNode_ApplyMeshSpaceAdditive::StaticStruct()) { return nullptr; }
    const UEdGraphNode* Visual = Class->GetVisualNodeFromNodePropertyIndex(PropertyIndex);
    if (Visual == nullptr || Visual->NodeGuid != NodeGuid) { return nullptr; }
    const FAnimNode_ApplyMeshSpaceAdditive* Template = Properties[PropertyIndex]->ContainerPtrToValuePtr<FAnimNode_ApplyMeshSpaceAdditive>(Class->GetDefaultObject());
    USkeleton* Skeleton = Interface->GetTargetSkeleton();
    const FReferenceSkeleton& Reference = Skeleton->GetReferenceSkeleton();
    if (Names->Num() != Reference.GetNum()) { return nullptr; }
    TArray<FBoneIndexType> BoneIndices;
    for (int32 Index = 0; Index < Names->Num(); ++Index)
    {
        FString Name;
        if (!(*Names)[Index]->TryGetString(Name) || Name != Reference.GetBoneName(Index).ToString()) { return nullptr; }
        BoneIndices.Add(Index);
    }
    struct FProbeProxy : FAnimInstanceProxy
    {
        using FAnimInstanceProxy::FAnimInstanceProxy;
        using FAnimInstanceProxy::Initialize;
    };
    TArray<TSharedPtr<FJsonValue>> Results;
    for (const TSharedPtr<FJsonValue>& Value : *Samples)
    {
        const TSharedPtr<FJsonObject>* Sample = nullptr;
        FString Name;
        double Alpha;
        if (!Value->TryGetObject(Sample) || !(*Sample)->TryGetStringField(TEXT("name"), Name) ||
            !(*Sample)->HasTypedField<EJson::Number>(TEXT("alpha")) || !(*Sample)->TryGetNumberField(TEXT("alpha"), Alpha) ||
            !FMath::IsFinite(static_cast<float>(Alpha)) || Alpha < 0.0 || Alpha > 1.0 ||
            !(*Sample)->HasField(TEXT("base")) || !(*Sample)->HasField(TEXT("additive"))) { return nullptr; }
        FSourcePoseProbeLeaf Base, Additive;
        if (!ReadSourcePoseProbeLeaf(Reference, (*Sample)->TryGetField(TEXT("base")), Base) ||
            !ReadSourcePoseProbeLeaf(Reference, (*Sample)->TryGetField(TEXT("additive")), Additive)) { return nullptr; }
        USkeletalMeshComponent* Mesh = NewObject<USkeletalMeshComponent>();
        UAnimInstance* Instance = NewObject<UAnimInstance>(Mesh, Class);
        FProbeProxy Proxy(Instance);
        Proxy.Initialize(Instance);
        if (!Proxy.IsSkeletonValid() || Proxy.GetSkeleton() != Skeleton) { return nullptr; }
        Proxy.GetRequiredBones().InitializeTo(BoneIndices, UE::Anim::FCurveFilterSettings(), *Skeleton);
        FAnimNode_ApplyMeshSpaceAdditive Node = *Template;
        Node.Base.SetLinkNode(&Base);
        Node.Additive.SetLinkNode(&Additive);
        Node.Initialize_AnyThread(FAnimationInitializeContext(&Proxy));
        Node.CacheBones_AnyThread(FAnimationCacheBonesContext(&Proxy));
        Node.ActualAlpha = static_cast<float>(Alpha);
        FPoseContext Pose(&Proxy);
        Node.Evaluate_AnyThread(Pose);
        const TSharedPtr<FJsonObject> Captured = CaptureCompactPose(Pose.Pose, Pose.Curve);
        if (!Captured.IsValid()) { return nullptr; }
        TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
        Row->SetStringField(TEXT("name"), Name);
        Row->SetNumberField(TEXT("alpha"), Node.ActualAlpha);
        Row->SetBoolField(TEXT("root_space_additive"), Node.bRootSpaceAdditive);
        Row->SetObjectField(TEXT("pose"), Captured.ToSharedRef());
        Results.Add(MakeShared<FJsonValueObject>(Row));
        UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Mesh additive sample %d/%d captured."), Results.Num(), Samples->Num());
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetStringField(TEXT("node_guid"), GuidText(NodeGuid));
    Result->SetStringField(TEXT("node_property"), Properties[PropertyIndex]->GetName());
    Result->SetArrayField(TEXT("samples"), Results);
    return Result;
}

TSharedPtr<FJsonObject> CaptureSlotProbe(UObject* Asset, const FString& RequestPath)
{
    FMemMark Mark(FMemStack::Get());
    UBlueprint* Blueprint = Cast<UBlueprint>(Asset);
    UAnimBlueprintGeneratedClass* Class = Blueprint != nullptr ? Cast<UAnimBlueprintGeneratedClass>(Blueprint->GeneratedClass) : nullptr;
    const IAnimClassInterface* Interface = Class != nullptr ? IAnimClassInterface::GetFromClass(Class) : nullptr;
    FString Text, NodeGuidText, SequencePath;
    TSharedPtr<FJsonObject> Request;
    const TSharedPtr<FJsonObject>* Dynamic = nullptr;
    const TArray<TSharedPtr<FJsonValue>>* Names = nullptr;
    const TArray<TSharedPtr<FJsonValue>>* Steps = nullptr;
    FGuid NodeGuid;
    if (Interface == nullptr || Interface->GetTargetSkeleton() == nullptr || !FFileHelper::LoadFileToString(Text, *RequestPath) ||
        !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) ||
        !Request->TryGetStringField(TEXT("node_guid"), NodeGuidText) || !FGuid::Parse(NodeGuidText, NodeGuid) || !NodeGuid.IsValid() ||
        !Request->TryGetArrayField(TEXT("bone_names"), Names) || !Request->TryGetArrayField(TEXT("steps"), Steps) ||
        Steps->IsEmpty() || Steps->Num() > 512 || !Request->TryGetObjectField(TEXT("dynamic_montage"), Dynamic) ||
        !(*Dynamic)->TryGetStringField(TEXT("sequence"), SequencePath)) { return nullptr; }
    const int32* NodeIndex = Class->GetNodePropertyIndexFromGuid(NodeGuid);
    const TArray<FStructProperty*>& Properties = Interface->GetAnimNodeProperties();
    const int32 PropertyIndex = NodeIndex != nullptr ? Properties.Num() - 1 - *NodeIndex : INDEX_NONE;
    if (!Properties.IsValidIndex(PropertyIndex) || Properties[PropertyIndex]->Struct != FAnimNode_Slot::StaticStruct()) { return nullptr; }
    const UEdGraphNode* Visual = Class->GetVisualNodeFromNodePropertyIndex(PropertyIndex);
    if (Visual == nullptr || Visual->NodeGuid != NodeGuid) { return nullptr; }
    const FAnimNode_Slot* Template = Properties[PropertyIndex]->ContainerPtrToValuePtr<FAnimNode_Slot>(Class->GetDefaultObject());
    USkeleton* Skeleton = Interface->GetTargetSkeleton();
    UAnimSequence* Sequence = LoadObject<UAnimSequence>(nullptr, *SequencePath);
    if (Sequence == nullptr || Sequence->GetSkeleton() != Skeleton) { return nullptr; }
    const FReferenceSkeleton& Reference = Skeleton->GetReferenceSkeleton();
    if (Names->Num() != Reference.GetNum()) { return nullptr; }
    TArray<FBoneIndexType> BoneIndices;
    for (int32 Index = 0; Index < Names->Num(); ++Index)
    {
        FString Name;
        if (!(*Names)[Index]->TryGetString(Name) || Name != Reference.GetBoneName(Index).ToString()) { return nullptr; }
        BoneIndices.Add(Index);
    }
    const auto ReadNumber = [](const TSharedPtr<FJsonObject>& Object, const TCHAR* Key, double& Value)
    {
        return Object->HasTypedField<EJson::Number>(Key) && Object->TryGetNumberField(Key, Value) && FMath::IsFinite(static_cast<float>(Value));
    };
    double BlendIn, BlendOut, Trigger, Rate, Start, PlayFrame;
    if (!ReadNumber(*Dynamic, TEXT("blend_in_seconds"), BlendIn) || BlendIn < 0.0 ||
        !ReadNumber(*Dynamic, TEXT("blend_out_seconds"), BlendOut) || BlendOut < 0.0 ||
        !ReadNumber(*Dynamic, TEXT("blend_out_trigger_seconds"), Trigger) ||
        !ReadNumber(*Dynamic, TEXT("play_rate"), Rate) || !ReadNumber(*Dynamic, TEXT("start_time"), Start) ||
        Start < 0.0 || Start > Sequence->GetPlayLength() || !ReadNumber(*Dynamic, TEXT("play_frame"), PlayFrame) ||
        PlayFrame < 0.0 || PlayFrame >= Steps->Num() || FMath::FloorToDouble(PlayFrame) != PlayFrame) { return nullptr; }
    Sequence->CacheDerivedDataForCurrentPlatform();
    UAnimMontage* Montage = UAnimMontage::CreateSlotAnimationAsDynamicMontage(Sequence, Template->SlotName, BlendIn, BlendOut, 1.f, 1, Trigger);
    if (Montage == nullptr) { return nullptr; }
    struct FProbeProxy : FAnimInstanceProxy
    {
        using FAnimInstanceProxy::FAnimInstanceProxy;
        using FAnimInstanceProxy::Initialize;
        using FAnimInstanceProxy::GetMontageEvaluationData;
    };
    struct FProbeSlot : FAnimNode_Slot
    {
        const FSlotNodeWeightInfo& Weights() const { return WeightData; }
    };
    USkeletalMeshComponent* Mesh = NewObject<USkeletalMeshComponent>();
    UAnimInstance* AnimInstance = NewObject<UAnimInstance>(Mesh, Class);
    FProbeProxy Proxy(AnimInstance);
    Proxy.Initialize(AnimInstance);
    if (!Proxy.IsSkeletonValid() || Proxy.GetSkeleton() != Skeleton) { return nullptr; }
    Proxy.GetRequiredBones().InitializeTo(BoneIndices, UE::Anim::FCurveFilterSettings(), *Skeleton);
    FSourcePoseProbeLeaf Leaf;
    FProbeSlot Node;
    static_cast<FAnimNode_Slot&>(Node) = *Template;
    Node.Source.SetLinkNode(&Leaf);
    Node.Initialize_AnyThread(FAnimationInitializeContext(&Proxy));
    Node.CacheBones_AnyThread(FAnimationCacheBonesContext(&Proxy));
    FAnimMontageInstance Playback;
    TArray<TSharedPtr<FJsonValue>> Results;
    for (int32 Frame = 0; Frame < Steps->Num(); ++Frame)
    {
        const TSharedPtr<FJsonObject>* Step = nullptr;
        const TSharedPtr<FJsonObject>* Curves = nullptr;
        const TArray<TSharedPtr<FJsonValue>>* Transforms = nullptr;
        double Delta;
        if (!(*Steps)[Frame]->TryGetObject(Step) || !ReadNumber(*Step, TEXT("delta_seconds"), Delta) || Delta < 0.0 ||
            !(*Step)->TryGetArrayField(TEXT("bone_transforms"), Transforms) || Transforms->Num() != Reference.GetNum() ||
            !(*Step)->TryGetObjectField(TEXT("curves"), Curves)) { return nullptr; }
        Leaf.Transforms.Reset();
        Leaf.Curves.Reset();
        for (const TSharedPtr<FJsonValue>& TransformValue : *Transforms)
        {
            const TSharedPtr<FJsonObject>* Transform = nullptr;
            if (!TransformValue->TryGetObject(Transform)) { return nullptr; }
            double Components[10];
            int32 Offset = 0;
            for (const TCHAR* Field : {TEXT("translation_cm"), TEXT("rotation"), TEXT("scale")})
            {
                const TArray<TSharedPtr<FJsonValue>>* Values = nullptr;
                const int32 Count = Offset == 3 ? 4 : 3;
                if (!(*Transform)->TryGetArrayField(Field, Values) || Values->Num() != Count) { return nullptr; }
                for (const TSharedPtr<FJsonValue>& Component : *Values)
                {
                    if (Component->Type != EJson::Number || !Component->TryGetNumber(Components[Offset]) || !FMath::IsFinite(Components[Offset])) { return nullptr; }
                    ++Offset;
                }
            }
            const FQuat Rotation(Components[3], Components[4], Components[5], Components[6]);
            if (!Rotation.IsNormalized()) { return nullptr; }
            Leaf.Transforms.Add(FTransform(Rotation, FVector(Components[0], Components[1], Components[2]), FVector(Components[7], Components[8], Components[9])));
        }
        for (const auto& Curve : (*Curves)->Values)
        {
            double Amount;
            if (Curve.Value->Type != EJson::Number || !Curve.Value->TryGetNumber(Amount) || !FMath::IsFinite(static_cast<float>(Amount))) { return nullptr; }
            Leaf.Curves.Add(FName(*Curve.Key), static_cast<float>(Amount));
        }
        if (Frame == static_cast<int32>(PlayFrame))
        {
            Playback.Initialize(Montage);
            Playback.Play(Rate);
            Playback.SetPosition(Start);
        }
        Proxy.GetMontageEvaluationData().Reset();
        if (Frame >= PlayFrame && Playback.IsValid())
        {
            Playback.UpdateWeight(Delta);
            Playback.Advance(Delta, nullptr, false);
            if (Playback.IsValid() && Playback.GetWeight() > ZERO_ANIMWEIGHT_THRESH)
            {
                Proxy.GetMontageEvaluationData().Emplace(Montage, Playback.GetPosition(), Playback.DeltaTimeRecord, Playback.IsPlaying(), Playback.IsActive(), Playback.GetBlend(), Playback.GetActiveBlendProfile(), Playback.GetBlendStartAlpha());
            }
        }
        FAnimationUpdateSharedContext Shared;
        Leaf.UpdateWeight = 0.f;
        Node.Update_AnyThread(FAnimationUpdateContext(&Proxy, static_cast<float>(Delta), &Shared));
        FPoseContext Pose(&Proxy);
        Node.Evaluate_AnyThread(Pose);
        TSharedPtr<FJsonObject> Row = CaptureCompactPose(Pose.Pose, Pose.Curve);
        if (!Row.IsValid()) { return nullptr; }
        Row->SetNumberField(TEXT("frame"), Frame);
        Row->SetNumberField(TEXT("source_update_weight"), Leaf.UpdateWeight);
        Row->SetNumberField(TEXT("slot_weight"), Node.Weights().SlotNodeWeight);
        Row->SetNumberField(TEXT("source_weight"), Node.Weights().SourceWeight);
        Row->SetNumberField(TEXT("total_weight"), Node.Weights().TotalNodeWeight);
        Row->SetNumberField(TEXT("montage_weight"), Playback.GetWeight());
        Row->SetNumberField(TEXT("montage_position"), Frame >= PlayFrame ? Playback.GetPosition() : 0.f);
        Results.Add(MakeShared<FJsonValueObject>(Row.ToSharedRef()));
        UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Slot probe frame %d/%d captured."), Frame + 1, Steps->Num());
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetStringField(TEXT("node_guid"), GuidText(NodeGuid));
    Result->SetStringField(TEXT("node_property"), Properties[PropertyIndex]->GetName());
    Result->SetStringField(TEXT("slot_name"), Template->SlotName.ToString());
    Result->SetArrayField(TEXT("frames"), Results);
    return Result;
}

TSharedPtr<FJsonObject> CaptureInertializationProbe(UObject* Asset, const FString& RequestPath)
{
    UBlueprint* Blueprint = Cast<UBlueprint>(Asset);
    const IAnimClassInterface* Interface = Blueprint != nullptr && Blueprint->GeneratedClass != nullptr
        ? IAnimClassInterface::GetFromClass(Blueprint->GeneratedClass) : nullptr;
    FString Text;
    TSharedPtr<FJsonObject> Request;
    const TArray<TSharedPtr<FJsonValue>>* Samples = nullptr;
    if (Interface == nullptr || Interface->GetTargetSkeleton() == nullptr ||
        !FFileHelper::LoadFileToString(Text, *RequestPath) ||
        !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) ||
        !Request->TryGetArrayField(TEXT("samples"), Samples) || Samples->IsEmpty() || Samples->Num() > 2048)
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Inertialization probe asset or request is invalid."));
        return nullptr;
    }
    FString RequestedNodeProperty;
    if (Request->HasField(TEXT("node_property")) &&
        (!Request->TryGetStringField(TEXT("node_property"), RequestedNodeProperty) || RequestedNodeProperty.IsEmpty())) { return nullptr; }
    FString NodeProperty;
    const FAnimNode_Inertialization* Template = nullptr;
    for (const FStructProperty* Property : Interface->GetAnimNodeProperties())
    {
        if (Property->Struct == FAnimNode_Inertialization::StaticStruct() &&
            (RequestedNodeProperty.IsEmpty() || RequestedNodeProperty == Property->GetName()))
        {
            if (Template != nullptr) { return nullptr; }
            Template = Property->ContainerPtrToValuePtr<FAnimNode_Inertialization>(Blueprint->GeneratedClass->GetDefaultObject());
            NodeProperty = Property->GetName();
        }
    }
    if (Template == nullptr) { return nullptr; }
    const auto ReadNumbers = [](const TSharedPtr<FJsonObject>& Object, const TCHAR* Key, double* Values, int32 Count)
    {
        const TArray<TSharedPtr<FJsonValue>>* Array = nullptr;
        if (!Object->TryGetArrayField(Key, Array) || Array->Num() != Count) { return false; }
        for (int32 Index = 0; Index < Count; ++Index)
        {
            if (!(*Array)[Index]->TryGetNumber(Values[Index]) || !FMath::IsFinite(Values[Index])) { return false; }
        }
        return true;
    };

    const FReferenceSkeleton& ReferenceSkeleton = Interface->GetTargetSkeleton()->GetReferenceSkeleton();
    const bool bFullPose = Request->HasField(TEXT("bone_names"));
    TArray<FBoneIndexType> RequiredBoneIndices;
    TArray<FString> BoneNames;
    if (bFullPose)
    {
        const TArray<TSharedPtr<FJsonValue>>* Names = nullptr;
        if (!Request->TryGetArrayField(TEXT("bone_names"), Names) || Names->Num() != ReferenceSkeleton.GetNum()) { return nullptr; }
        for (int32 Index = 0; Index < Names->Num(); ++Index)
        {
            FString Name;
            if (!(*Names)[Index]->TryGetString(Name) || Name != ReferenceSkeleton.GetBoneName(Index).ToString()) { return nullptr; }
            BoneNames.Add(Name);
            RequiredBoneIndices.Add(Index);
        }
    }
    else { RequiredBoneIndices.Add(0); }
    const auto ReadTransform = [&ReadNumbers](const TSharedPtr<FJsonObject>& Object, FTransform& Transform)
    {
        double Translation[3], Rotation[4], Scale[3];
        if (!ReadNumbers(Object, TEXT("translation_cm"), Translation, 3) ||
            !ReadNumbers(Object, TEXT("rotation"), Rotation, 4) || !ReadNumbers(Object, TEXT("scale"), Scale, 3)) { return false; }
        const FQuat Quaternion(Rotation[0], Rotation[1], Rotation[2], Rotation[3]);
        if (!Quaternion.IsNormalized()) { return false; }
        Transform = FTransform(Quaternion, FVector(Translation[0], Translation[1], Translation[2]), FVector(Scale[0], Scale[1], Scale[2]));
        return true;
    };
    struct FProbeProxy : FAnimInstanceProxy
    {
        using FAnimInstanceProxy::FAnimInstanceProxy;
        using FAnimInstanceProxy::Initialize;
        void AdvanceFrame() { UpdateCounter.Increment(); }
    };
    struct FProbeNode : FAnimNode_Base
    {
        TArray<FTransform> Transforms;
        TArray<FCompactPoseBoneIndex> BoneIndices;
        TMap<FName, float> Curves;
        virtual void Evaluate_AnyThread(FPoseContext& Output) override
        {
            Output.Pose.ResetToRefPose();
            for (int32 Index = 0; Index < Transforms.Num(); ++Index) { Output.Pose[BoneIndices[Index]] = Transforms[Index]; }
            Output.Curve.Empty();
            for (const auto& Curve : Curves) { Output.Curve.Set(Curve.Key, Curve.Value); }
        }
    };
    TArray<TSharedPtr<FJsonValue>> Results;
    for (const TSharedPtr<FJsonValue>& Value : *Samples)
    {
        const TSharedPtr<FJsonObject>* Sample = nullptr;
        const TArray<TSharedPtr<FJsonValue>>* Steps = nullptr;
        FString Name;
        if (!Value->TryGetObject(Sample) || !(*Sample)->TryGetStringField(TEXT("name"), Name) ||
            !(*Sample)->TryGetArrayField(TEXT("steps"), Steps) || Steps->IsEmpty() || Steps->Num() > 256) { return nullptr; }
        USkeletalMeshComponent* Mesh = NewObject<USkeletalMeshComponent>();
        Mesh->SetTeleportDistanceThreshold(0.0f);
        UAnimInstance* Instance = NewObject<UAnimInstance>(Mesh);
        FProbeProxy Proxy(Instance);
        Proxy.Initialize(Instance);
        Proxy.GetRequiredBones().InitializeTo(RequiredBoneIndices, UE::Anim::FCurveFilterSettings(), *Interface->GetTargetSkeleton());
        FProbeNode Child;
        Child.Transforms.SetNum(RequiredBoneIndices.Num());
        for (const FBoneIndexType Index : RequiredBoneIndices)
        {
            const FCompactPoseBoneIndex Compact = Proxy.GetRequiredBones().GetCompactPoseIndexFromSkeletonPoseIndex(FSkeletonPoseBoneIndex(Index));
            if (Compact.GetInt() == INDEX_NONE) { return nullptr; }
            Child.BoneIndices.Add(Compact);
        }
        FAnimNode_Inertialization Node(*Template);
        Node.Source.SetLinkNode(&Child);
        Node.Initialize_AnyThread(FAnimationInitializeContext(&Proxy));
        Node.CacheBones_AnyThread(FAnimationCacheBonesContext(&Proxy));
        TArray<TSharedPtr<FJsonValue>> Frames;
        for (const TSharedPtr<FJsonValue>& StepValue : *Steps)
        {
            const TSharedPtr<FJsonObject>* Step = nullptr;
            double Delta;
            if (!StepValue->TryGetObject(Step) || !(*Step)->TryGetNumberField(TEXT("delta_seconds"), Delta) ||
                !FMath::IsFinite(static_cast<float>(Delta)) || Delta < 0.0) { return nullptr; }
            if (bFullPose)
            {
                const TArray<TSharedPtr<FJsonValue>>* Transforms = nullptr;
                if ((*Step)->HasField(TEXT("translation_cm")) || !(*Step)->TryGetArrayField(TEXT("bone_transforms"), Transforms) ||
                    Transforms->Num() != Child.Transforms.Num()) { return nullptr; }
                for (int32 Index = 0; Index < Transforms->Num(); ++Index)
                {
                    const TSharedPtr<FJsonObject>* Transform = nullptr;
                    if (!(*Transforms)[Index]->TryGetObject(Transform) || !ReadTransform(*Transform, Child.Transforms[Index])) { return nullptr; }
                }
            }
            else if ((*Step)->HasField(TEXT("bone_transforms")) || !ReadTransform(*Step, Child.Transforms[0])) { return nullptr; }
            Child.Curves.Reset();
            const TSharedPtr<FJsonObject>* Curves = nullptr;
            if ((*Step)->HasField(TEXT("curves")))
            {
                if (!(*Step)->TryGetObjectField(TEXT("curves"), Curves)) { return nullptr; }
                for (const auto& Curve : (*Curves)->Values)
                {
                    double Amount;
                    if (!Curve.Value->TryGetNumber(Amount) || !FMath::IsFinite(static_cast<float>(Amount))) { return nullptr; }
                    Child.Curves.Add(FName(*Curve.Key), static_cast<float>(Amount));
                }
            }
            if ((*Step)->HasField(TEXT("request_duration")))
            {
                double Duration;
                if (!(*Step)->TryGetNumberField(TEXT("request_duration"), Duration) ||
                    !FMath::IsFinite(static_cast<float>(Duration)) || Duration < 0.0) { return nullptr; }
                Node.RequestInertialization(static_cast<float>(Duration), nullptr);
            }
            if ((*Step)->HasField(TEXT("request_durations")))
            {
                const TArray<TSharedPtr<FJsonValue>>* Durations = nullptr;
                if (!(*Step)->TryGetArrayField(TEXT("request_durations"), Durations)) { return nullptr; }
                for (const TSharedPtr<FJsonValue>& DurationValue : *Durations)
                {
                    double Duration;
                    if (!DurationValue->TryGetNumber(Duration) || !FMath::IsFinite(static_cast<float>(Duration)) || Duration < 0.0) { return nullptr; }
                    Node.RequestInertialization(static_cast<float>(Duration), nullptr);
                }
            }
            Proxy.AdvanceFrame();
            FAnimationUpdateSharedContext SharedContext;
            Node.Update_AnyThread(FAnimationUpdateContext(&Proxy, static_cast<float>(Delta), &SharedContext));
            FMemMark Mark(FMemStack::Get());
            FPoseContext Pose(&Proxy);
            Node.Evaluate_AnyThread(Pose);
            TSharedRef<FJsonObject> Frame = MakeShared<FJsonObject>();
            Frame->SetObjectField(TEXT("transform"), CaptureTransform(Pose.Pose[FCompactPoseBoneIndex(0)]));
            if (bFullPose)
            {
                TArray<TSharedPtr<FJsonValue>> Transforms;
                for (const FCompactPoseBoneIndex Index : Child.BoneIndices)
                {
                    if (Pose.Pose[Index].ContainsNaN()) { return nullptr; }
                    Transforms.Add(MakeShared<FJsonValueObject>(CaptureTransform(Pose.Pose[Index])));
                }
                Frame->SetArrayField(TEXT("bone_transforms"), Transforms);
            }
            TSharedRef<FJsonObject> CurveValues = MakeShared<FJsonObject>();
            Pose.Curve.ForEachElement([&CurveValues](const UE::Anim::FCurveElement& Curve) { CurveValues->SetNumberField(Curve.Name.ToString(), Curve.Value); });
            Frame->SetObjectField(TEXT("curves"), CurveValues);
            Frames.Add(MakeShared<FJsonValueObject>(Frame));
        }
        TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
        Result->SetStringField(TEXT("name"), Name);
        Result->SetArrayField(TEXT("frames"), Frames);
        Results.Add(MakeShared<FJsonValueObject>(Result));
        if (Results.Num() % 100 == 0) { UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Inertialization samples completed: %d"), Results.Num()); }
    }
    TSharedRef<FJsonObject> Probe = MakeShared<FJsonObject>();
    Probe->SetArrayField(TEXT("samples"), Results);
    Probe->SetStringField(TEXT("node_property"), NodeProperty);
    if (bFullPose)
    {
        TArray<TSharedPtr<FJsonValue>> Names, Parents;
        for (int32 Index = 0; Index < BoneNames.Num(); ++Index)
        {
            Names.Add(MakeShared<FJsonValueString>(BoneNames[Index]));
            Parents.Add(MakeShared<FJsonValueNumber>(ReferenceSkeleton.GetParentIndex(Index)));
        }
        Probe->SetArrayField(TEXT("bone_names"), Names);
        Probe->SetArrayField(TEXT("parent_indices"), Parents);
    }
    Probe->SetNumberField(TEXT("enabled"), IConsoleManager::Get().FindConsoleVariable(TEXT("a.AnimNode.Inertialization.Enable"))->GetInt());
    Probe->SetNumberField(TEXT("ignore_velocity"), IConsoleManager::Get().FindConsoleVariable(TEXT("a.AnimNode.Inertialization.IgnoreVelocity"))->GetInt());
    Probe->SetNumberField(TEXT("ignore_deficit"), IConsoleManager::Get().FindConsoleVariable(TEXT("a.AnimNode.Inertialization.IgnoreDeficit"))->GetInt());
    return Probe;
}

TSharedPtr<FJsonObject> CaptureMontageClockProbe(UObject* Asset, const FString& RequestPath)
{
    UAnimMontage* Montage = Cast<UAnimMontage>(Asset);
    FString Text;
    TSharedPtr<FJsonObject> Request;
    const TArray<TSharedPtr<FJsonValue>>* Samples = nullptr;
    if (!FFileHelper::LoadFileToString(Text, *RequestPath) ||
        !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) ||
        !Request->TryGetArrayField(TEXT("samples"), Samples) || Samples->IsEmpty())
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Montage clock probe asset or request is invalid."));
        return nullptr;
    }
    if (Montage == nullptr)
    {
        UAnimSequenceBase* Sequence = Cast<UAnimSequenceBase>(Asset);
        const TSharedPtr<FJsonObject>* Dynamic = nullptr;
        FString Slot;
        double BlendIn, BlendOut, Trigger;
        if (Sequence == nullptr || !Request->TryGetObjectField(TEXT("dynamic_montage"), Dynamic) ||
            !(*Dynamic)->TryGetStringField(TEXT("slot"), Slot) || Slot.IsEmpty() ||
            !(*Dynamic)->TryGetNumberField(TEXT("blend_in_seconds"), BlendIn) || !FMath::IsFinite(static_cast<float>(BlendIn)) || BlendIn < 0.0 ||
            !(*Dynamic)->TryGetNumberField(TEXT("blend_out_seconds"), BlendOut) || !FMath::IsFinite(static_cast<float>(BlendOut)) || BlendOut < 0.0 ||
            !(*Dynamic)->TryGetNumberField(TEXT("blend_out_trigger_seconds"), Trigger) || !FMath::IsFinite(static_cast<float>(Trigger)))
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Dynamic montage clock configuration is invalid."));
            return nullptr;
        }
        Montage = UAnimMontage::CreateSlotAnimationAsDynamicMontage(Sequence, FName(*Slot), BlendIn, BlendOut, 1.0f, 1, Trigger);
        if (Montage == nullptr) { return nullptr; }
        bool bInertialBlendOut = false;
        if ((*Dynamic)->HasField(TEXT("inertial_blend_out")) && !(*Dynamic)->TryGetBoolField(TEXT("inertial_blend_out"), bInertialBlendOut)) { return nullptr; }
        if (bInertialBlendOut) { Montage->BlendModeOut = EMontageBlendMode::Inertialization; }
    }
    TArray<TSharedPtr<FJsonValue>> Results;
    for (const TSharedPtr<FJsonValue>& Value : *Samples)
    {
        const TSharedPtr<FJsonObject>* Sample = nullptr;
        const TArray<TSharedPtr<FJsonValue>>* Steps = nullptr;
        FString Name;
        if (!Value->TryGetObject(Sample) || !(*Sample)->TryGetStringField(TEXT("name"), Name) ||
            !(*Sample)->TryGetArrayField(TEXT("steps"), Steps) || Steps->IsEmpty())
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Montage clock probe sample is invalid."));
            return nullptr;
        }
        FAnimMontageInstance Instance;
        Instance.Initialize(Montage);
        double PlayRate = 1.0, StartTime = 0.0;
        if (((*Sample)->HasField(TEXT("play_rate")) && !(*Sample)->TryGetNumberField(TEXT("play_rate"), PlayRate)) ||
            ((*Sample)->HasField(TEXT("start_time")) && !(*Sample)->TryGetNumberField(TEXT("start_time"), StartTime)) ||
            !FMath::IsFinite(static_cast<float>(PlayRate)) || !FMath::IsFinite(static_cast<float>(StartTime))) { return nullptr; }
        Instance.Play(PlayRate);
        Instance.SetPosition(FMath::Clamp(static_cast<float>(StartTime), 0.0f, Montage->GetPlayLength()));
        TArray<TSharedPtr<FJsonValue>> Frames;
        for (const TSharedPtr<FJsonValue>& StepValue : *Steps)
        {
            const TSharedPtr<FJsonObject>* Step = nullptr;
            double Delta;
            if (!StepValue->TryGetObject(Step) || !(*Step)->TryGetNumberField(TEXT("delta_seconds"), Delta) ||
                !FMath::IsFinite(Delta) || Delta < 0.0)
            {
                UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Montage clock probe delta is invalid."));
                return nullptr;
            }
            double StopDuration;
            if ((*Step)->TryGetNumberField(TEXT("stop_seconds"), StopDuration))
            {
                if (!FMath::IsFinite(StopDuration) || StopDuration < 0.0) { return nullptr; }
                bool bActiveOnly = false;
                bool bInertial = Montage->BlendModeOut == EMontageBlendMode::Inertialization;
                if (((*Step)->HasField(TEXT("active_only_stop")) && !(*Step)->TryGetBoolField(TEXT("active_only_stop"), bActiveOnly))
                    || ((*Step)->HasField(TEXT("inertial_stop")) && !(*Step)->TryGetBoolField(TEXT("inertial_stop"), bInertial))) { return nullptr; }
                if (Instance.IsValid() && (!bActiveOnly || Instance.IsActive()))
                {
                    FMontageBlendSettings Settings(FAlphaBlend(Montage->BlendOut, StopDuration));
                    Settings.BlendMode = bInertial ? EMontageBlendMode::Inertialization : EMontageBlendMode::Standard;
                    Settings.BlendProfile = Montage->BlendProfileOut;
                    Instance.Stop(Settings);
                }
            }
            Instance.UpdateWeight(Delta);
            Instance.Advance(Delta, nullptr, false);
            TSharedRef<FJsonObject> Frame = MakeShared<FJsonObject>();
            Frame->SetNumberField(TEXT("delta_seconds"), static_cast<float>(Delta));
            Frame->SetNumberField(TEXT("position"), Instance.GetPosition());
            Frame->SetNumberField(TEXT("weight"), Instance.GetWeight());
            Frame->SetBoolField(TEXT("valid"), Instance.IsValid());
            Frame->SetBoolField(TEXT("playing"), Instance.IsPlaying());
            Frame->SetBoolField(TEXT("stopped"), Instance.IsStopped());
            Frames.Add(MakeShared<FJsonValueObject>(Frame));
        }
        TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
        Result->SetStringField(TEXT("name"), Name);
        Result->SetArrayField(TEXT("frames"), Frames);
        Results.Add(MakeShared<FJsonValueObject>(Result));
    }
    TSharedRef<FJsonObject> Probe = MakeShared<FJsonObject>();
    Probe->SetArrayField(TEXT("samples"), Results);
    Probe->SetNumberField(TEXT("montage_length"), Montage->GetPlayLength());
    return Probe;
}

TSharedPtr<FJsonObject> CaptureNotifyClockProbe(UObject* Asset)
{
    UAnimSequenceBase* Sequence = Cast<UAnimSequenceBase>(Asset);
    if (Sequence == nullptr || Sequence->GetPlayLength() <= 0.0f)
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Notify clock probe requires a non-empty animation sequence."));
        return nullptr;
    }
    const float Length = Sequence->GetPlayLength();
    TArray<TSharedPtr<FJsonValue>> Results;
    for (const bool Looped : {false, true})
    {
        for (const float Periods : {0.2f, -0.2f, 1.2f, -1.2f, 2.6f, -2.6f})
        {
            const float Start = Length * 0.6f;
            const float Delta = Length * Periods;
            float Time = Start;
            FMarkerTickRecord MarkerRecord;
            FDeltaTimeRecord DeltaRecord;
            DeltaRecord.Set(Start, Delta);
            FAnimTickRecord TickRecord(Sequence, Looped, 1.0f, false, 1.0f, Time, MarkerRecord);
            TickRecord.DeltaTimeRecord = &DeltaRecord;
            FAnimNotifyContext Context(TickRecord);
            Sequence->GetAnimNotifies(Start, Delta, Context);
            TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
            Result->SetBoolField(TEXT("looping"), Looped);
            Result->SetNumberField(TEXT("start_time"), Start);
            Result->SetNumberField(TEXT("delta_seconds"), Delta);
            TArray<TSharedPtr<FJsonValue>> Events;
            for (const FAnimNotifyEventReference& Reference : Context.ActiveNotifies)
            {
                const FAnimNotifyEvent* Notify = Reference.GetNotify();
                check(Notify != nullptr);
                TSharedRef<FJsonObject> Event = MakeShared<FJsonObject>();
                Event->SetNumberField(TEXT("event_index"), Sequence->Notifies.IndexOfByPredicate(
                    [Notify](const FAnimNotifyEvent& Candidate) { return &Candidate == Notify; }));
                Event->SetNumberField(TEXT("trigger_time"), Notify->GetTriggerTime());
                Event->SetNumberField(TEXT("end_trigger_time"), Notify->GetEndTriggerTime());
                Events.Add(MakeShared<FJsonValueObject>(Event));
            }
            Result->SetArrayField(TEXT("events"), Events);
            Results.Add(MakeShared<FJsonValueObject>(Result));
        }
    }
    TSharedRef<FJsonObject> Probe = MakeShared<FJsonObject>();
    Probe->SetNumberField(TEXT("duration"), Length);
    Probe->SetArrayField(TEXT("cases"), Results);
    return Probe;
}

TSharedRef<FJsonObject> CaptureNotifyChanceProbe(UObject* Asset)
{
    FAnimNotifyQueue Queue;
    FAnimNotifyEvent Event;
    const auto Accept = [&Event, Asset](FAnimNotifyQueue& Target)
    {
        Target.AnimNotifies.Reset();
        Target.AddAnimNotifies(true, {FAnimNotifyEventReference(&Event, Asset)}, 1.0f);
        return !Target.AnimNotifies.IsEmpty();
    };
    TArray<TSharedPtr<FJsonValue>> Results;
    for (const float Chance : {0.0f, 1.0f, 0.25f, 0.5f, 0.75f, 0.0f, 1.0f, 0.5f})
    {
        Event.NotifyTriggerChance = Chance;
        TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
        Result->SetNumberField(TEXT("chance"), Chance);
        Result->SetBoolField(TEXT("accepted"), Accept(Queue));
        Result->SetNumberField(TEXT("seed"), Queue.RandomStream.GetCurrentSeed());
        Results.Add(MakeShared<FJsonValueObject>(Result));
    }
    TSharedRef<FJsonObject> Probe = MakeShared<FJsonObject>();
    Probe->SetArrayField(TEXT("points"), Results);
    Probe->SetNumberField(TEXT("seed_before_state"), Queue.RandomStream.GetCurrentSeed());
    Event.NotifyTriggerChance = 0.0f;
    Event.NotifyStateClass = GetMutableDefault<UAnimNotifyState>();
    Probe->SetBoolField(TEXT("state_accepted"), Accept(Queue));
    Probe->SetNumberField(TEXT("seed_after_state"), Queue.RandomStream.GetCurrentSeed());
    Queue.AnimNotifies.Reset();
    Queue.UnfilteredMontageAnimNotifies.Reset();
    Probe->SetNumberField(TEXT("seed_after_clear_events"), Queue.RandomStream.GetCurrentSeed());
    Event.NotifyStateClass = nullptr;
    Event.NotifyTriggerChance = 0.5f;
    Probe->SetBoolField(TEXT("after_clear_accepted"), Accept(Queue));
    Probe->SetNumberField(TEXT("after_clear_seed"), Queue.RandomStream.GetCurrentSeed());
    FAnimNotifyQueue Fresh;
    Probe->SetBoolField(TEXT("fresh_accepted"), Accept(Fresh));
    Probe->SetNumberField(TEXT("fresh_seed"), Fresh.RandomStream.GetCurrentSeed());
    return Probe;
}

TSharedPtr<FJsonObject> CaptureNotifyDispatchProbe(UObject* Asset)
{
    UAnimSequenceBase* Sequence = Cast<UAnimSequenceBase>(Asset);
    if (Sequence == nullptr) { return nullptr; }
    const FAnimNotifyEvent* State = Sequence->Notifies.FindByPredicate(
        [](const FAnimNotifyEvent& Event) { return Event.NotifyStateClass != nullptr; });
    if (State == nullptr) { return nullptr; }
    USkeletalMeshComponent* Mesh = NewObject<USkeletalMeshComponent>();
    UAnimInstance* Instance = NewObject<UAnimInstance>(Mesh);
    TGuardValue<uint64> FrameGuard(GFrameCounter, 0);
    check(Mesh->PoseTickedThisFrame());
    FAnimNotifyEventReference Reference(State, Sequence);
    Instance->NotifyQueue.AddAnimNotifies({Reference}, 1.0f);
    TArray<TSharedPtr<FJsonValue>> Rows;
    const auto Capture = [&Rows, Instance](const TCHAR* Stage)
    {
        TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
        Row->SetStringField(TEXT("stage"), Stage);
        Row->SetNumberField(TEXT("active_state_count"), Instance->ActiveAnimNotifyState.Num());
        Row->SetNumberField(TEXT("queued_count"), Instance->NotifyQueue.AnimNotifies.Num());
        Rows.Add(MakeShared<FJsonValueObject>(Row));
    };
    Mesh->bIsAutonomousTickPose = true;
    Instance->DispatchQueuedAnimEvents();
    Capture(TEXT("autonomous"));
    Mesh->bIsAutonomousTickPose = false;
    Instance->ClearQueuedAnimEvents(true);
    Instance->DispatchQueuedAnimEvents();
    Capture(TEXT("first_component_only"));
    Instance->ClearQueuedAnimEvents(true);
    Instance->DispatchQueuedAnimEvents();
    Capture(TEXT("second_component_only"));
    Instance->NotifyQueue.AddAnimNotifies({Reference}, 1.0f);
    Mesh->bIsAutonomousTickPose = true;
    Instance->DispatchQueuedAnimEvents();
    Capture(TEXT("autonomous_again"));
    Mesh->bIsAutonomousTickPose = false;
    Instance->ClearQueuedAnimEvents(true);
    Instance->TriggerAnimNotifies(0.1f, UAnimInstance::ENotifyTriggerMode::ForceMontageOnly);
    Capture(TEXT("forced_montage_only"));
    TSharedRef<FJsonObject> Probe = MakeShared<FJsonObject>();
    Probe->SetArrayField(TEXT("rows"), Rows);
    Probe->SetNumberField(TEXT("dispatch_policy"),
        IConsoleManager::Get().FindConsoleVariable(TEXT("a.Notifies.DispatchPolicy"))->GetInt());
    Probe->SetBoolField(TEXT("pose_ticked_this_frame"), Mesh->PoseTickedThisFrame());
    return Probe;
}

TSharedRef<FJsonObject> CaptureAlphaBlendProbe()
{
    TArray<TSharedPtr<FJsonValue>> Rows;
    const EAlphaBlendOption Modes[] = {EAlphaBlendOption::Cubic, EAlphaBlendOption::HermiteCubic, EAlphaBlendOption::QuadraticInOut};
    for (const EAlphaBlendOption Mode : Modes)
    {
        for (const float Alpha : {0.0f, 0.25f, 0.5f, 0.75f, 0.9997720122337341f, 0.9997889995574951f, 1.0f})
        {
            const float Delta = Alpha * 0.2f;
            const float QueryAlpha = Delta / 0.2f;
            FAlphaBlend Blend;
            Blend.SetBlendTime(0.2f);
            Blend.SetBlendOption(Mode);
            Blend.SetValueRange(0.0f, 1.0f);
            Blend.Update(Delta);
            TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
            Row->SetStringField(TEXT("mode"), Mode == EAlphaBlendOption::Cubic ? TEXT("Cubic") : Mode == EAlphaBlendOption::HermiteCubic ? TEXT("HermiteCubic") : TEXT("QuadraticInOut"));
            Row->SetNumberField(TEXT("delta"), Delta);
            Row->SetNumberField(TEXT("query_alpha"), QueryAlpha);
            Row->SetNumberField(TEXT("curve_value"), FAlphaBlend::AlphaToBlendOption(QueryAlpha, Mode, nullptr));
            Row->SetNumberField(TEXT("blend_value"), Blend.GetBlendedValue());
            Row->SetBoolField(TEXT("complete"), Blend.IsComplete());
            Rows.Add(MakeShared<FJsonValueObject>(Row));
        }
        UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Alpha blend probe mode %d/3 completed."), static_cast<int32>(Rows.Num()) / 7);
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetStringField(TEXT("format"), TEXT("ue_alpha_blend_probe_v1"));
    Result->SetArrayField(TEXT("rows"), Rows);
    TArray<TSharedPtr<FJsonValue>> Sequences;
    for (const EAlphaBlendOption Mode : Modes)
    {
        for (const FVector2f Timing : {FVector2f(0.1f, 1.0f / 60.0f), FVector2f(0.1f, 0.02f), FVector2f(0.8f, 0.02f)})
        {
            const float Duration = Timing.X;
            const float Delta = Timing.Y;
            FAlphaBlend Blend;
            Blend.SetBlendTime(Duration);
            Blend.SetBlendOption(Mode);
            Blend.SetValueRange(0.0f, 1.0f);
            float Elapsed = 0.0f;
            TArray<TSharedPtr<FJsonValue>> Frames;
            for (int32 Frame = 1; Frame <= FMath::CeilToInt(Duration / Delta) + 2; ++Frame)
            {
                Elapsed += Delta;
                Blend.Update(Delta);
                TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
                Row->SetNumberField(TEXT("frame"), Frame);
                Row->SetNumberField(TEXT("elapsed"), Elapsed);
                Row->SetNumberField(TEXT("query_alpha"), Elapsed / Duration);
                Row->SetNumberField(TEXT("curve_value"), FAlphaBlend::AlphaToBlendOption(Elapsed / Duration, Mode, nullptr));
                Row->SetNumberField(TEXT("alpha_lerp"), Blend.GetAlpha());
                Row->SetNumberField(TEXT("blend_value"), Blend.GetBlendedValue());
                Row->SetBoolField(TEXT("complete"), Blend.IsComplete());
                Frames.Add(MakeShared<FJsonValueObject>(Row));
            }
            TSharedRef<FJsonObject> Sequence = MakeShared<FJsonObject>();
            Sequence->SetStringField(TEXT("mode"), Mode == EAlphaBlendOption::Cubic ? TEXT("Cubic") : Mode == EAlphaBlendOption::HermiteCubic ? TEXT("HermiteCubic") : TEXT("QuadraticInOut"));
            Sequence->SetNumberField(TEXT("duration"), Duration);
            Sequence->SetNumberField(TEXT("delta"), Delta);
            Sequence->SetArrayField(TEXT("frames"), Frames);
            Sequences.Add(MakeShared<FJsonValueObject>(Sequence));
        }
        UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Alpha blend sequence mode %d/3 completed."), static_cast<int32>(Sequences.Num()) / 3);
    }
    Result->SetArrayField(TEXT("sequences"), Sequences);
    return Result;
}

TSharedPtr<FJsonObject> CaptureStateMachineEntryProbe(UObject* Asset, const FString& RequestPath)
{
    UBlueprint* Blueprint = Cast<UBlueprint>(Asset);
    UClass* Class = Blueprint != nullptr ? Blueprint->GeneratedClass : nullptr;
    const IAnimClassInterface* Interface = Class != nullptr ? IAnimClassInterface::GetFromClass(Class) : nullptr;
    FString Text;
    TSharedPtr<FJsonObject> Request;
    const TArray<TSharedPtr<FJsonValue>>* Samples = nullptr;
    double MachineValue;
    if (Interface == nullptr || !FFileHelper::LoadFileToString(Text, *RequestPath) || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) ||
        !Request->TryGetNumberField(TEXT("machine_index"), MachineValue) || !FMath::IsFinite(MachineValue) || MachineValue != FMath::FloorToDouble(MachineValue) ||
        MachineValue < 0 || MachineValue >= Interface->GetBakedStateMachines().Num() || !Request->TryGetArrayField(TEXT("samples"), Samples) || Samples->IsEmpty() || Samples->Num() > 32) { return nullptr; }
    const int32 MachineIndex = static_cast<int32>(MachineValue);
    const FBakedAnimationStateMachine& Description = Interface->GetBakedStateMachines()[MachineIndex];
    const TArray<FStructProperty*>& Properties = Interface->GetAnimNodeProperties();
    const FAnimNode_StateMachine* Template = nullptr;
    for (const FStructProperty* Property : Interface->GetStateMachineNodeProperties())
    {
        const FAnimNode_StateMachine* Node = Property->ContainerPtrToValuePtr<FAnimNode_StateMachine>(Class->GetDefaultObject());
        if (Node->StateMachineIndexInClass == MachineIndex) { Template = Node; }
    }
    if (Template == nullptr) { return nullptr; }
    TSet<int32> Required;
    for (const FBakedAnimationState& State : Description.States)
    {
        if (State.EntryRuleNodeIndex != INDEX_NONE) { Required.Add(State.EntryRuleNodeIndex); }
        for (const FBakedStateExitTransition& Exit : State.Transitions) { if (Exit.CanTakeDelegateIndex != INDEX_NONE) { Required.Add(Exit.CanTakeDelegateIndex); } }
    }
    struct FProbeLeaf : FAnimNode_StateResult
    {
        TArray<float> Weights;
        virtual void Initialize_AnyThread(const FAnimationInitializeContext& Context) override {}
        virtual void CacheBones_AnyThread(const FAnimationCacheBonesContext& Context) override {}
        virtual void Update_AnyThread(const FAnimationUpdateContext& Context) override { Weights.Add(Context.GetFinalBlendWeight()); }
    };
    struct FProbeMachine : FAnimNode_StateMachine
    {
        TArray<FProbeLeaf>* ProbeLeaves = nullptr;
        virtual void Initialize_AnyThread(const FAnimationInitializeContext& Context) override
        {
            FAnimNode_StateMachine::Initialize_AnyThread(Context);
            if (ProbeLeaves != nullptr) { Link(*ProbeLeaves); }
        }
        void Link(TArray<FProbeLeaf>& Leaves) { ProbeLeaves = &Leaves; for (int32 Index = 0; Index < Leaves.Num(); ++Index) { StatePoseLinks[Index].SetLinkNode(&Leaves[Index]); StatePoseLinks[Index].LinkID = INDEX_NONE; } }
        int32 ActiveCount() const { return ActiveTransitionArray.Num(); }
        float Weight(int32 Index) const { return GetStateWeight(Index); }
        TArray<TSharedPtr<FJsonValue>> CaptureActiveEntries() const
        {
            TArray<TSharedPtr<FJsonValue>> Entries;
            for (const FAnimationActiveTransitionEntry& Transition : ActiveTransitionArray)
            {
                TSharedRef<FJsonObject> Entry = MakeShared<FJsonObject>();
                Entry->SetNumberField(TEXT("source"), Transition.PreviousState);
                Entry->SetNumberField(TEXT("target"), Transition.NextState);
                Entry->SetNumberField(TEXT("alpha"), Transition.Alpha);
                TArray<TSharedPtr<FJsonValue>> Indices;
                for (int32 Index : Transition.SourceTransitionIndices) { Indices.Add(MakeShared<FJsonValueNumber>(Index)); }
                Entry->SetArrayField(TEXT("indices"), Indices);
                Entries.Add(MakeShared<FJsonValueObject>(Entry));
            }
            return Entries;
        }
    };
    struct FProbeProxy : FAnimInstanceProxy
    {
        explicit FProbeProxy(UAnimInstance* Instance) : FAnimInstanceProxy(Instance) {}
        void BeginGraph(UAnimInstance* Instance) { Initialize(Instance); InitializationCounter.Increment(); }
        void BeginUpdate() { UpdateCounter.Increment(); }
    };
    TArray<TSharedPtr<FJsonValue>> Rows;
    for (const TSharedPtr<FJsonValue>& SampleValue : *Samples)
    {
        const TSharedPtr<FJsonObject>* Sample = nullptr;
        const TArray<TSharedPtr<FJsonValue>>* Steps = nullptr;
        FString Name;
        double Limit = Template->MaxTransitionsPerFrame;
        if (!SampleValue->TryGetObject(Sample) || !(*Sample)->TryGetStringField(TEXT("name"), Name) || !(*Sample)->TryGetArrayField(TEXT("steps"), Steps) || Steps->IsEmpty() || Steps->Num() > 512 ||
            ((*Sample)->HasField(TEXT("max_transitions")) && !(*Sample)->TryGetNumberField(TEXT("max_transitions"), Limit)) || !FMath::IsFinite(Limit) || Limit != FMath::FloorToDouble(Limit) || Limit < 1 || Limit > 64) { return nullptr; }
        UWorld* World = NewObject<UWorld>(GetTransientPackage());
        USkeletalMeshComponent* Mesh = NewObject<USkeletalMeshComponent>(World);
        UAnimInstance* Instance = NewObject<UAnimInstance>(Mesh, Class);
        FProbeProxy Proxy(Instance);
        Proxy.BeginGraph(Instance);
        TMap<int32, bool> Conditions;
        for (int32 Index : Required)
        {
            const int32 PropertyIndex = Properties.Num() - 1 - Index;
            if (!Properties.IsValidIndex(PropertyIndex) || Properties[PropertyIndex]->Struct != FAnimNode_TransitionResult::StaticStruct()) { return nullptr; }
            FAnimNode_TransitionResult* Node = Properties[PropertyIndex]->ContainerPtrToValuePtr<FAnimNode_TransitionResult>(Instance);
            Node->NativeTransitionDelegate.BindLambda([&Conditions, Index]() { return Conditions.FindChecked(Index); });
        }
        FProbeMachine Machine;
        Machine.StateMachineIndexInClass = MachineIndex;
        Machine.MaxTransitionsPerFrame = static_cast<int32>(Limit);
        Machine.bSkipFirstUpdateTransition = Template->bSkipFirstUpdateTransition;
        Machine.bReinitializeOnBecomingRelevant = Template->bReinitializeOnBecomingRelevant;
        Machine.bAllowConduitEntryStates = Template->bAllowConduitEntryStates;
        Machine.bCreateNotifyMetaData = false;
        Machine.CacheMachineDescription(const_cast<IAnimClassInterface*>(Interface));
        Machine.Initialize_AnyThread(FAnimationInitializeContext(&Proxy));
        TArray<FProbeLeaf> Leaves;
        Leaves.SetNum(Description.States.Num());
        Machine.Link(Leaves);
        const TSharedPtr<FJsonObject>* FirstStep = nullptr;
        if (!(*Steps)[0]->TryGetObject(FirstStep)) { return nullptr; }
        const bool NativeAutomatic = (*FirstStep)->HasField(TEXT("asset_players"));
        TArray<TSharedPtr<FJsonValue>> Frames;
        for (const TSharedPtr<FJsonValue>& StepValue : *Steps)
        {
            const TSharedPtr<FJsonObject>* Step = nullptr;
            const TArray<TSharedPtr<FJsonValue>>* StateEntries = nullptr;
            const TArray<TSharedPtr<FJsonValue>>* Transitions = nullptr;
            double Delta;
            bool UpdateMachine = true;
            if (!StepValue->TryGetObject(Step) || (*Step)->HasField(TEXT("asset_players")) != NativeAutomatic || !(*Step)->TryGetNumberField(TEXT("delta_seconds"), Delta) || !FMath::IsFinite(static_cast<float>(Delta)) || Delta < 0 || Delta > 1 ||
                ((*Step)->HasField(TEXT("update_machine")) && (!(*Step)->HasTypedField<EJson::Boolean>(TEXT("update_machine")) || !(*Step)->TryGetBoolField(TEXT("update_machine"), UpdateMachine))) ||
                !(*Step)->TryGetArrayField(TEXT("state_entries"), StateEntries) || StateEntries->Num() != Description.States.Num() ||
                !(*Step)->TryGetArrayField(TEXT("transition_rules"), Transitions) || Transitions->Num() != Description.Transitions.Num()) { return nullptr; }
            Conditions.Reset();
            for (int32 Index = 0; Index < Description.States.Num(); ++Index)
            {
                const FBakedAnimationState& State = Description.States[Index];
                bool Entry;
                if ((*StateEntries)[Index]->Type != EJson::Boolean || !(*StateEntries)[Index]->TryGetBool(Entry)) { return nullptr; }
                if (State.EntryRuleNodeIndex != INDEX_NONE) { Conditions.Add(State.EntryRuleNodeIndex, Entry); }
                for (const FBakedStateExitTransition& Exit : State.Transitions)
                {
                    bool Result;
                    if (Exit.CanTakeDelegateIndex == INDEX_NONE) { continue; }
                    if (!Transitions->IsValidIndex(Exit.TransitionIndex) || (*Transitions)[Exit.TransitionIndex]->Type != EJson::Boolean || !(*Transitions)[Exit.TransitionIndex]->TryGetBool(Result) ||
                        (Conditions.Contains(Exit.CanTakeDelegateIndex) && Conditions[Exit.CanTakeDelegateIndex] != Result)) { return nullptr; }
                    Conditions.Add(Exit.CanTakeDelegateIndex, Result);
                }
            }
            if ((*Step)->HasField(TEXT("asset_players")))
            {
                const TArray<TSharedPtr<FJsonValue>>* Players = nullptr;
                if (!(*Step)->TryGetArrayField(TEXT("asset_players"), Players) || Players->Num() > Properties.Num()) { return nullptr; }
                for (const FStructProperty* Property : Properties)
                {
                    if (Property->Struct->IsChildOf(FAnimNode_AssetPlayerBase::StaticStruct()))
                    {
                        Property->ContainerPtrToValuePtr<FAnimNode_AssetPlayerBase>(Instance)->ClearCachedBlendWeight();
                    }
                }
                TSet<int32> Seen;
                const FFloatProperty* WeightProperty = FindFProperty<FFloatProperty>(FAnimNode_AssetPlayerBase::StaticStruct(), TEXT("BlendWeight"));
                if (WeightProperty == nullptr) { return nullptr; }
                for (const TSharedPtr<FJsonValue>& PlayerValue : *Players)
                {
                    const TSharedPtr<FJsonObject>* Player = nullptr;
                    double Index, Weight, Time;
                    if (!PlayerValue->TryGetObject(Player) || !(*Player)->TryGetNumberField(TEXT("node_index"), Index) || !FMath::IsFinite(Index) || Index != FMath::FloorToDouble(Index) ||
                        Index < 0 || Index >= Properties.Num() || !(*Player)->TryGetNumberField(TEXT("weight"), Weight) || !FMath::IsFinite(static_cast<float>(Weight)) || Weight < 0 || Weight > 1 ||
                        !(*Player)->TryGetNumberField(TEXT("position"), Time) || !FMath::IsFinite(static_cast<float>(Time)) || Time < 0) { return nullptr; }
                    const int32 NodeIndex = static_cast<int32>(Index);
                    const FStructProperty* Property = Properties[Properties.Num() - 1 - NodeIndex];
                    if (Seen.Contains(NodeIndex) || !Property->Struct->IsChildOf(FAnimNode_SequencePlayer::StaticStruct())) { return nullptr; }
                    Seen.Add(NodeIndex);
                    FAnimNode_SequencePlayer* Node = Property->ContainerPtrToValuePtr<FAnimNode_SequencePlayer>(Instance);
                    if (Node->IsLooping() || Node->GetAnimAsset() == nullptr || Time > Node->GetAnimAsset()->GetPlayLength()) { return nullptr; }
                    WeightProperty->SetPropertyValue_InContainer(Node, static_cast<float>(Weight));
                    Node->SetAccumulatedTime(static_cast<float>(Time));
                }
                for (const FBakedAnimationState& State : Description.States)
                {
                    for (const FBakedStateExitTransition& Exit : State.Transitions)
                    {
                        if (Exit.bAutomaticRemainingTimeRule && Exit.CanTakeDelegateIndex != INDEX_NONE)
                        {
                            Properties[Properties.Num() - 1 - Exit.CanTakeDelegateIndex]->ContainerPtrToValuePtr<FAnimNode_TransitionResult>(Instance)->NativeTransitionDelegate.Unbind();
                        }
                    }
                }
            }
            for (FProbeLeaf& Leaf : Leaves) { Leaf.Weights.Reset(); }
            Proxy.BeginUpdate();
            FAnimationUpdateSharedContext SharedContext;
            if (UpdateMachine) { Machine.Update_AnyThread(FAnimationUpdateContext(&Proxy, static_cast<float>(Delta), &SharedContext)); }
            TSharedRef<FJsonObject> Frame = MakeShared<FJsonObject>();
            Frame->SetBoolField(TEXT("updated"), UpdateMachine);
            Frame->SetNumberField(TEXT("state"), Machine.GetCurrentState());
            Frame->SetStringField(TEXT("name"), Machine.GetCurrentStateName().ToString());
            Frame->SetNumberField(TEXT("elapsed"), Machine.GetCurrentStateElapsedTime());
            Frame->SetNumberField(TEXT("active_transitions"), Machine.ActiveCount());
            Frame->SetArrayField(TEXT("active_entries"), Machine.CaptureActiveEntries());
            TArray<TSharedPtr<FJsonValue>> Weights, Updates;
            for (int32 Index = 0; Index < Leaves.Num(); ++Index)
            {
                Weights.Add(MakeShared<FJsonValueNumber>(Machine.Weight(Index)));
                TArray<TSharedPtr<FJsonValue>> Calls;
                for (float Weight : Leaves[Index].Weights) { Calls.Add(MakeShared<FJsonValueNumber>(Weight)); }
                Updates.Add(MakeShared<FJsonValueArray>(Calls));
            }
            Frame->SetArrayField(TEXT("weights"), Weights);
            Frame->SetArrayField(TEXT("updates"), Updates);
            Frames.Add(MakeShared<FJsonValueObject>(Frame));
        }
        TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
        Row->SetStringField(TEXT("name"), Name);
        Row->SetNumberField(TEXT("max_transitions"), Limit);
        Row->SetArrayField(TEXT("frames"), Frames);
        Rows.Add(MakeShared<FJsonValueObject>(Row));
        UE_LOG(LogAlsGraphSourceReader, Display, TEXT("State machine entry sample %d/%d captured."), Rows.Num(), Samples->Num());
    }
    TSharedRef<FJsonObject> Probe = MakeShared<FJsonObject>();
    Probe->SetNumberField(TEXT("machine_index"), MachineIndex);
    Probe->SetArrayField(TEXT("samples"), Rows);
    return Probe;
}

TSharedPtr<FJsonObject> CaptureStateMachinePlayers(UObject* Asset)
{
    UBlueprint* Blueprint = Cast<UBlueprint>(Asset);
    UAnimBlueprintGeneratedClass* Class = Blueprint != nullptr ? Cast<UAnimBlueprintGeneratedClass>(Blueprint->GeneratedClass) : nullptr;
    const IAnimClassInterface* Interface = Class != nullptr ? IAnimClassInterface::GetFromClass(Class) : nullptr;
    if (Interface == nullptr) { return nullptr; }
    const TArray<FStructProperty*>& Properties = Interface->GetAnimNodeProperties();
    const auto CaptureNode = [&](int32 Index) -> TSharedPtr<FJsonObject>
    {
        if (!Properties.IsValidIndex(Index)) { return nullptr; }
        const int32 PropertyIndex = Properties.Num() - 1 - Index;
        const UEdGraphNode* Visual = Class->GetVisualNodeFromNodePropertyIndex(PropertyIndex);
        const int32* RoundTrip = Visual != nullptr ? Class->GetNodePropertyIndexFromGuid(Visual->NodeGuid) : nullptr;
        const UEdGraphNode* RoundTripVisual = RoundTrip != nullptr && Properties.IsValidIndex(*RoundTrip)
            ? Class->GetVisualNodeFromNodePropertyIndex(Properties.Num() - 1 - *RoundTrip) : nullptr;
        if (Visual == nullptr || RoundTripVisual != Visual)
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Compiled node identity mismatch: node=%d property=%s visual=%s round_trip=%d."),
                Index, *Properties[PropertyIndex]->GetName(), *GetPathNameSafe(Visual), RoundTrip == nullptr ? INDEX_NONE : *RoundTrip);
            return nullptr;
        }
        const FStructProperty* Property = Properties[PropertyIndex];
        TSharedRef<FJsonObject> Node = MakeShared<FJsonObject>();
        Node->SetNumberField(TEXT("node_index"), Index);
        if (*RoundTrip != Index) { Node->SetNumberField(TEXT("shared_origin_node_index"), *RoundTrip); }
        Node->SetStringField(TEXT("property"), Property->GetName());
        Node->SetStringField(TEXT("struct"), Property->Struct->GetPathName());
        Node->SetStringField(TEXT("guid"), GuidText(Visual->NodeGuid));
        Node->SetStringField(TEXT("source_path"), Visual->GetPathName());
        Node->SetStringField(TEXT("source_graph"), Visual->GetGraph()->GetPathName());
        if (Property->Struct->IsChildOf(FAnimNode_AssetPlayerBase::StaticStruct()))
        {
            const FAnimNode_AssetPlayerBase* Player = Property->ContainerPtrToValuePtr<FAnimNode_AssetPlayerBase>(Class->GetDefaultObject());
            UAnimationAsset* Animation = Player->GetAnimAsset();
            TSharedRef<FJsonObject> Sync = MakeShared<FJsonObject>();
            Sync->SetStringField(TEXT("group"), Player->GetGroupName().ToString());
            Sync->SetStringField(TEXT("role"), StaticEnum<EAnimGroupRole::Type>()->GetNameStringByValue(Player->GetGroupRole()));
            Sync->SetStringField(TEXT("method"), StaticEnum<EAnimSyncMethod>()->GetNameStringByValue(static_cast<int64>(Player->GetGroupMethod())));
            Sync->SetBoolField(TEXT("override_position"), Player->GetOverridePositionWhenJoiningSyncGroupAsLeader());
            Sync->SetBoolField(TEXT("asset_is_null"), Animation == nullptr);
            Sync->SetStringField(TEXT("asset_path"), ObjectPath(Animation));
            TArray<TSharedPtr<FJsonValue>> Markers;
            if (const TArray<FName>* Names = Animation != nullptr ? Animation->GetUniqueMarkerNames() : nullptr)
            {
                for (const FName Name : *Names) { Markers.Add(MakeShared<FJsonValueString>(Name.ToString())); }
            }
            Sync->SetArrayField(TEXT("marker_names"), Markers);
            Node->SetObjectField(TEXT("synchronization"), Sync);
        }
        if (Property->Struct == FAnimNode_SequencePlayer::StaticStruct())
        {
            const FAnimNode_SequencePlayer* Player = Property->ContainerPtrToValuePtr<FAnimNode_SequencePlayer>(Class->GetDefaultObject());
            TSharedRef<FJsonObject> Effective = MakeShared<FJsonObject>();
            Effective->SetNumberField(TEXT("play_rate"), Player->GetPlayRate());
            Effective->SetNumberField(TEXT("basis"), Player->GetPlayRateBasis());
            Effective->SetNumberField(TEXT("start"), Player->GetStartPosition());
            Effective->SetBoolField(TEXT("loop"), Player->IsLooping());
            Effective->SetBoolField(TEXT("ignore"), Player->GetIgnoreForRelevancyTest());
            Node->SetObjectField(TEXT("effective"), Effective);
        }
        if (const FObjectPropertyBase* SequenceProperty = FindFProperty<FObjectPropertyBase>(Property->Struct, TEXT("Sequence")))
        {
            if (const UAnimSequenceBase* Sequence = Cast<UAnimSequenceBase>(SequenceProperty->GetObjectPropertyValue_InContainer(Property->ContainerPtrToValuePtr<void>(Class->GetDefaultObject()))))
            {
                Node->SetStringField(TEXT("asset_path"), Sequence->GetPathName());
                Node->SetNumberField(TEXT("asset_duration"), Sequence->GetPlayLength());
                Node->SetNumberField(TEXT("asset_rate_scale"), Sequence->RateScale);
            }
        }
        return Node;
    };
    TArray<TSharedPtr<FJsonValue>> Machines;
    const TArray<FBakedAnimationStateMachine>& Descriptions = Interface->GetBakedStateMachines();
    for (int32 MachineIndex = 0; MachineIndex < Descriptions.Num(); ++MachineIndex)
    {
        const FBakedAnimationStateMachine& Description = Descriptions[MachineIndex];
        const FStateMachineDebugData* MachineDebug = nullptr;
        for (const auto& Entry : Class->GetAnimBlueprintDebugData().StateMachineDebugData)
        {
            if (Entry.Value.MachineIndex != MachineIndex) { continue; }
            if (MachineDebug != nullptr) { return nullptr; }
            MachineDebug = &Entry.Value;
        }
        if (MachineDebug == nullptr) { return nullptr; }
        TArray<TSharedPtr<FJsonValue>> States;
        for (int32 StateIndex = 0; StateIndex < Description.States.Num(); ++StateIndex)
        {
            const FBakedAnimationState& Info = Description.States[StateIndex];
            TSharedRef<FJsonObject> State = MakeShared<FJsonObject>();
            State->SetNumberField(TEXT("index"), StateIndex);
            State->SetStringField(TEXT("name"), Info.StateName.ToString());
            State->SetBoolField(TEXT("conduit"), Info.bIsAConduit);
            State->SetBoolField(TEXT("always_reset"), Info.bAlwaysResetOnEntry);
            TArray<TSharedPtr<FJsonValue>> Exits;
            for (const FBakedStateExitTransition& Exit : Info.Transitions)
            {
                if (!Description.Transitions.IsValidIndex(Exit.TransitionIndex)) { return nullptr; }
                const FAnimationTransitionBetweenStates& Transition = Description.Transitions[Exit.TransitionIndex];
                const TSharedPtr<FJsonObject> Rule = CaptureNode(Exit.CanTakeDelegateIndex);
                if (!Rule.IsValid()) { return nullptr; }
                const UEdGraphNode* TransitionNode = MachineDebug->FindNodeFromTransitionIndex(Exit.TransitionIndex);
                if (TransitionNode == nullptr) { return nullptr; }
                TSharedRef<FJsonObject> Edge = MakeShared<FJsonObject>();
                Edge->SetNumberField(TEXT("index"), Exit.TransitionIndex);
                Edge->SetStringField(TEXT("transition_guid"), GuidText(TransitionNode->NodeGuid));
                Edge->SetStringField(TEXT("transition_path"), TransitionNode->GetPathName());
                Edge->SetNumberField(TEXT("source"), Transition.PreviousState);
                Edge->SetNumberField(TEXT("target"), Transition.NextState);
                Edge->SetNumberField(TEXT("logic"), static_cast<int32>(Transition.LogicType.GetValue()));
                Edge->SetNumberField(TEXT("blend_mode"), static_cast<int32>(Transition.BlendMode));
                Edge->SetNumberField(TEXT("duration"), Transition.CrossfadeDuration);
                Edge->SetNumberField(TEXT("min_reentry_time"), Transition.MinTimeBeforeReentry);
                Edge->SetBoolField(TEXT("inertial_on_self"), Transition.bAllowInertializationForSelfTransitions);
                Edge->SetStringField(TEXT("profile"), GetPathNameSafe(Transition.BlendProfile));
                Edge->SetBoolField(TEXT("automatic"), Exit.bAutomaticRemainingTimeRule);
                Edge->SetBoolField(TEXT("only_active"), Exit.bOnlyEvaluateWhenActive);
                Edge->SetBoolField(TEXT("desired_result"), Exit.bDesiredTransitionReturnValue);
                Edge->SetNumberField(TEXT("trigger"), Exit.AutomaticRuleTriggerTime);
                Edge->SetStringField(TEXT("required_sync_group"), Exit.SyncGroupNameToRequireValidMarkersRule.ToString());
                Edge->SetObjectField(TEXT("rule"), Rule.ToSharedRef());
                Exits.Add(MakeShared<FJsonValueObject>(Edge));
            }
            State->SetArrayField(TEXT("exits"), Exits);
            if (Info.StateRootNodeIndex != INDEX_NONE)
            {
                const TSharedPtr<FJsonObject> Root = CaptureNode(Info.StateRootNodeIndex);
                if (!Root.IsValid()) { return nullptr; }
                State->SetObjectField(TEXT("root"), Root.ToSharedRef());
            }
            TArray<TSharedPtr<FJsonValue>> Players, Layers;
            for (int32 Index : Info.PlayerNodeIndices)
            {
                const TSharedPtr<FJsonObject> Node = CaptureNode(Index);
                if (!Node.IsValid()) { return nullptr; }
                Players.Add(MakeShared<FJsonValueObject>(Node));
            }
            for (int32 Index : Info.LayerNodeIndices)
            {
                const TSharedPtr<FJsonObject> Node = CaptureNode(Index);
                if (!Node.IsValid()) { return nullptr; }
                Layers.Add(MakeShared<FJsonValueObject>(Node));
            }
            State->SetArrayField(TEXT("players"), Players);
            State->SetArrayField(TEXT("layers"), Layers);
            States.Add(MakeShared<FJsonValueObject>(State));
        }
        TSharedRef<FJsonObject> Machine = MakeShared<FJsonObject>();
        Machine->SetNumberField(TEXT("index"), MachineIndex);
        Machine->SetStringField(TEXT("name"), Description.MachineName.ToString());
        Machine->SetNumberField(TEXT("initial_state"), Description.InitialState);
        Machine->SetArrayField(TEXT("states"), States);
        Machines.Add(MakeShared<FJsonValueObject>(Machine));
        UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Compiled state machine %d/%d captured."), MachineIndex + 1, Descriptions.Num());
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetArrayField(TEXT("machines"), Machines);
    TArray<TSharedPtr<FJsonValue>> AssetPlayers;
    for (int32 Index = 0; Index < Properties.Num(); ++Index)
    {
        if (!Properties[Properties.Num() - 1 - Index]->Struct->IsChildOf(FAnimNode_AssetPlayerBase::StaticStruct())) { continue; }
        const TSharedPtr<FJsonObject> Node = CaptureNode(Index);
        if (!Node.IsValid()) { return nullptr; }
        AssetPlayers.Add(MakeShared<FJsonValueObject>(Node));
    }
    Result->SetArrayField(TEXT("asset_players"), AssetPlayers);
    return Result;
}

TSharedPtr<FJsonObject> CaptureCachedPoseSchedule(UBlueprint* Blueprint)
{
    UClass* Class = Blueprint != nullptr ? Blueprint->GeneratedClass : nullptr;
    const IAnimClassInterface* Interface = Class != nullptr ? IAnimClassInterface::GetFromClass(Class) : nullptr;
    if (Interface == nullptr)
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Cached pose schedule requires an animation blueprint."));
        return nullptr;
    }
    UObject* DefaultObject = Class->GetDefaultObject();
    const TArray<FStructProperty*>& Properties = Interface->GetAnimNodeProperties();
    TArray<TSharedPtr<FJsonValue>> Layers;
    for (const TPair<FName, FCachedPoseIndices>& Pair : Interface->GetOrderedSavedPoseNodeIndicesMap())
    {
        TArray<TSharedPtr<FJsonValue>> Nodes;
        for (const int32 Index : Pair.Value.OrderedSavedPoseNodeIndices)
        {
            const int32 PropertyIndex = Properties.Num() - 1 - Index;
            if (!Properties.IsValidIndex(PropertyIndex) || !Properties[PropertyIndex]->Struct->IsChildOf(FAnimNode_SaveCachedPose::StaticStruct()))
            {
                UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Cached pose schedule contains an invalid property index: %d."), Index);
                return nullptr;
            }
            const FAnimNode_SaveCachedPose* Node = Properties[PropertyIndex]->ContainerPtrToValuePtr<FAnimNode_SaveCachedPose>(DefaultObject);
            TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
            Row->SetNumberField(TEXT("node_index"), Index);
            Row->SetNumberField(TEXT("property_index"), PropertyIndex);
            Row->SetStringField(TEXT("property_name"), Properties[PropertyIndex]->GetName());
            Row->SetStringField(TEXT("cache_name"), Node->CachePoseName.ToString());
            Nodes.Add(MakeShared<FJsonValueObject>(Row));
        }
        TSharedRef<FJsonObject> Layer = MakeShared<FJsonObject>();
        Layer->SetStringField(TEXT("name"), Pair.Key.ToString());
        Layer->SetArrayField(TEXT("nodes"), Nodes);
        Layers.Add(MakeShared<FJsonValueObject>(Layer));
        UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Cached pose schedule layer %s captured %d nodes."), *Pair.Key.ToString(), Nodes.Num());
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetStringField(TEXT("format"), TEXT("ue_cached_pose_schedule_v1"));
    Result->SetArrayField(TEXT("layers"), Layers);
    return Result;
}

TSharedRef<FJsonObject> CaptureCachedPoseLifecycleProbe()
{
    struct FProbeProxy : FAnimInstanceProxy
    {
        void InitializeGraph() { InitializationCounter.Increment(); }
        void AdvanceFrame() { UpdateCounter.Increment(); }
    } Proxy;
    struct FProbeNode : FAnimNode_Base
    {
        int32 Initializations = 0;
        int32 Updates = 0;
        float Position = 0.0f;
        TArray<float> Weights;
        virtual void Initialize_AnyThread(const FAnimationInitializeContext& Context) override
        {
            ++Initializations;
            Position = 0.2f;
        }
        virtual void Update_AnyThread(const FAnimationUpdateContext& Context) override
        {
            ++Updates;
            Position += Context.GetDeltaTime();
            Weights.Add(Context.GetFinalBlendWeight());
        }
    } Child;
    struct FProbeCache : FAnimNode_SaveCachedPose
    {
        bool WasUpdated() const { return UpdateCounter.HasEverBeenUpdated(); }
    } Cache;
    Cache.Pose.SetLinkNode(&Child);
    Proxy.InitializeGraph();
    TArray<TSharedPtr<FJsonValue>> Frames;
    for (int32 Index = 0; Index < 9; ++Index)
    {
        Proxy.AdvanceFrame();
        const bool bActive = Index != 4 && Index != 5;
        const bool bInitialize = Index == 0 || Index == 2 || Index == 6 || Index == 7 || Index == 8;
        if (Index == 8)
        {
            Proxy.InitializeGraph();
        }
        if (bInitialize)
        {
            Cache.Initialize_AnyThread(FAnimationInitializeContext(&Proxy));
        }
        if (bActive)
        {
            Cache.Update_AnyThread(FAnimationUpdateContext(&Proxy, 0.1f));
        }
        Cache.PostGraphUpdate();
        TSharedRef<FJsonObject> Frame = MakeShared<FJsonObject>();
        Frame->SetNumberField(TEXT("frame"), Index);
        Frame->SetBoolField(TEXT("active"), bActive);
        Frame->SetBoolField(TEXT("initialize_request"), bInitialize);
        Frame->SetBoolField(TEXT("instance_reinitialize"), Index == 8);
        Frame->SetBoolField(TEXT("cache_update_counter_valid"), Cache.WasUpdated());
        Frame->SetNumberField(TEXT("child_initializations"), Child.Initializations);
        Frame->SetNumberField(TEXT("child_updates"), Child.Updates);
        Frame->SetNumberField(TEXT("child_position"), Child.Position);
        Frames.Add(MakeShared<FJsonValueObject>(Frame));
        UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Cached pose lifecycle probe frame %d/9."), Index + 1);
    }
    TSharedRef<FJsonObject> Result = MakeShared<FJsonObject>();
    Result->SetStringField(TEXT("format"), TEXT("ue_cached_pose_lifecycle_probe_v1"));
    Result->SetArrayField(TEXT("frames"), Frames);
    TArray<TSharedPtr<FJsonValue>> WeightCases;
    const TArray<TArray<float>> Requests = { {0.9999999403953552f, 1.0f}, {1.0f, 0.9999999403953552f}, {0.2f, 0.8f}, {0.0f}, {} };
    for (const TArray<float>& Weights : Requests)
    {
        Proxy.AdvanceFrame();
        FProbeNode Leaf;
        FProbeCache Saved;
        Saved.Pose.SetLinkNode(&Leaf);
        Saved.Initialize_AnyThread(FAnimationInitializeContext(&Proxy));
        FAnimationUpdateSharedContext SharedContext;
        const FAnimationUpdateContext Context(&Proxy, 0.1f, &SharedContext);
        TArray<TSharedPtr<FJsonValue>> Inputs, Outputs;
        for (float Weight : Weights)
        {
            Inputs.Add(MakeShared<FJsonValueNumber>(Weight));
            Saved.Update_AnyThread(Context.FractionalWeight(Weight));
        }
        Saved.PostGraphUpdate();
        for (float Weight : Leaf.Weights) { Outputs.Add(MakeShared<FJsonValueNumber>(Weight)); }
        TSharedRef<FJsonObject> Entry = MakeShared<FJsonObject>();
        Entry->SetArrayField(TEXT("requests"), Inputs);
        Entry->SetArrayField(TEXT("child_update_weights"), Outputs);
        WeightCases.Add(MakeShared<FJsonValueObject>(Entry));
    }
    Result->SetArrayField(TEXT("weight_cases"), WeightCases);
    return Result;
}

UAlsGraphSourceReaderCommandlet::UAlsGraphSourceReaderCommandlet()
{
    IsClient = false;
    IsEditor = true;
    IsServer = false;
    LogToConsole = true;
}

int32 UAlsGraphSourceReaderCommandlet::Main(const FString& Params)
{
    FString AssetPath;
    FString OutputPath;
    FString BlendSpaceProbePath;
    FString SequencePoseProbePath;
    FParse::Value(*Params, TEXT("Asset="), AssetPath);
    FParse::Value(*Params, TEXT("Output="), OutputPath);
    FParse::Value(*Params, TEXT("BlendSpaceProbe="), BlendSpaceProbePath);
    FParse::Value(*Params, TEXT("SequencePoseProbe="), SequencePoseProbePath);

    if (AssetPath.IsEmpty() || OutputPath.IsEmpty())
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Asset and Output parameters are required."));
        return 2;
    }

    if (!AssetPath.Contains(TEXT(".")))
    {
        const FString AssetName = FPaths::GetBaseFilename(AssetPath);
        AssetPath = FString::Printf(TEXT("%s.%s"), *AssetPath, *AssetName);
    }

    UObject* Asset = LoadObject<UObject>(nullptr, *AssetPath);
    if (!IsValid(Asset))
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Unable to load asset %s."), *AssetPath);
        return 3;
    }

    UBlueprint* Blueprint = Cast<UBlueprint>(Asset);
    TArray<UObject*> AssetObjects;
    GetObjectsWithOuter(Asset, AssetObjects, EGetObjectsFlags::IncludeNestedObjects);
    AssetObjects.Add(Asset);
    AssetObjects.Sort([](const UObject& Left, const UObject& Right)
    {
        return Left.GetPathName() < Right.GetPathName();
    });

    TArray<UObject*> PackageObjects;
    GetObjectsWithOuter(Asset->GetOutermost(), PackageObjects, EGetObjectsFlags::IncludeNestedObjects);

    TArray<UEdGraph*> Graphs;
    for (UObject* Object : PackageObjects)
    {
        if (UEdGraph* Graph = Cast<UEdGraph>(Object))
        {
            Graphs.Add(Graph);
        }
    }

    Graphs.Sort([](const UEdGraph& Left, const UEdGraph& Right)
    {
        return Left.GetPathName() < Right.GetPathName();
    });

    TSharedRef<FJsonObject> Root = MakeShared<FJsonObject>();
    Root->SetStringField(TEXT("format"), Blueprint != nullptr ? TEXT("ue_animation_blueprint_graph_v1") : TEXT("ue_asset_source_v1"));
    Root->SetStringField(TEXT("asset_path"), Asset->GetPathName());
    Root->SetStringField(TEXT("asset_class"), Asset->GetClass()->GetPathName());
    Root->SetArrayField(TEXT("asset_properties"), CaptureProperties(Asset->GetClass(), Asset, Asset));
    if (UClass* Class = Cast<UClass>(Asset))
    {
        UObject* DefaultObject = Class->GetDefaultObject();
        Root->SetArrayField(TEXT("class_default_properties"), CaptureProperties(Class, DefaultObject, DefaultObject));
    }
    if (UStaticMesh* Mesh = Cast<UStaticMesh>(Asset))
    {
        FTriMeshCollisionData Data;
        const bool bAvailable = Mesh->HasValidRenderData() && Mesh->GetPhysicsTriMeshData(&Data, false);
        TSharedRef<FJsonObject> Collision = MakeShared<FJsonObject>();
        Collision->SetBoolField(TEXT("available"), bAvailable);
        Collision->SetBoolField(TEXT("flip_normals"), Data.bFlipNormals);
        TArray<TSharedPtr<FJsonValue>> Vertices;
        for (const FVector3f& Vertex : Data.Vertices)
        {
            Vertices.Add(MakeShared<FJsonValueObject>(CaptureVector(FVector(Vertex))));
        }
        TArray<TSharedPtr<FJsonValue>> Triangles;
        for (const FTriIndices& Triangle : Data.Indices)
        {
            TArray<TSharedPtr<FJsonValue>> Indices;
            Indices.Add(MakeShared<FJsonValueNumber>(Triangle.v0));
            Indices.Add(MakeShared<FJsonValueNumber>(Triangle.v1));
            Indices.Add(MakeShared<FJsonValueNumber>(Triangle.v2));
            Triangles.Add(MakeShared<FJsonValueArray>(Indices));
        }
        Collision->SetArrayField(TEXT("vertices_source"), Vertices);
        Collision->SetArrayField(TEXT("triangle_indices"), Triangles);
        Root->SetObjectField(TEXT("static_mesh_collision"), Collision);
        if (Mesh->HasValidRenderData() && Mesh->GetRenderData()->LODResources.Num() > 0)
        {
            const FStaticMeshLODResources& LOD = Mesh->GetRenderData()->LODResources[0];
            TSharedRef<FJsonObject> Geometry = MakeShared<FJsonObject>();
            TArray<TSharedPtr<FJsonValue>> Positions;
            TArray<TSharedPtr<FJsonValue>> Normals;
            TArray<TSharedPtr<FJsonValue>> UVChannels;
            for (uint32 Vertex = 0; Vertex < LOD.VertexBuffers.PositionVertexBuffer.GetNumVertices(); ++Vertex)
            {
                Positions.Add(MakeShared<FJsonValueObject>(CaptureVector(FVector(LOD.VertexBuffers.PositionVertexBuffer.VertexPosition(Vertex)))));
                Normals.Add(MakeShared<FJsonValueObject>(CaptureVector(FVector(LOD.VertexBuffers.StaticMeshVertexBuffer.VertexTangentZ(Vertex)))));
            }
            for (uint32 Channel = 0; Channel < LOD.VertexBuffers.StaticMeshVertexBuffer.GetNumTexCoords(); ++Channel)
            {
                TArray<TSharedPtr<FJsonValue>> UVs;
                for (uint32 Vertex = 0; Vertex < LOD.VertexBuffers.StaticMeshVertexBuffer.GetNumVertices(); ++Vertex)
                {
                    const FVector2f UV = LOD.VertexBuffers.StaticMeshVertexBuffer.GetVertexUV(Vertex, Channel);
                    TArray<TSharedPtr<FJsonValue>> Values;
                    Values.Add(MakeShared<FJsonValueNumber>(UV.X));
                    Values.Add(MakeShared<FJsonValueNumber>(UV.Y));
                    UVs.Add(MakeShared<FJsonValueArray>(Values));
                }
                UVChannels.Add(MakeShared<FJsonValueArray>(UVs));
            }
            TArray<TSharedPtr<FJsonValue>> RenderTriangles;
            const FIndexArrayView Indices = LOD.IndexBuffer.GetArrayView();
            for (int32 Index = 0; Index + 2 < Indices.Num(); Index += 3)
            {
                TArray<TSharedPtr<FJsonValue>> Values;
                for (int32 Corner = 0; Corner < 3; ++Corner) { Values.Add(MakeShared<FJsonValueNumber>(Indices[Index + Corner])); }
                RenderTriangles.Add(MakeShared<FJsonValueArray>(Values));
            }
            Geometry->SetArrayField(TEXT("vertices_source"), Positions);
            Geometry->SetArrayField(TEXT("normals_source"), Normals);
            Geometry->SetArrayField(TEXT("uv_channels"), UVChannels);
            Geometry->SetArrayField(TEXT("triangle_indices"), RenderTriangles);
            Geometry->SetObjectField(TEXT("bounds_origin_source"), CaptureVector(Mesh->GetBounds().Origin));
            Geometry->SetNumberField(TEXT("bounds_radius_source"), Mesh->GetBounds().SphereRadius);
            Root->SetObjectField(TEXT("static_mesh_geometry"), Geometry);
        }
    }
    if (UTexture2D* Texture = Cast<UTexture2D>(Asset))
    {
        FImage SourceImage;
        if (!Texture->Source.GetMipImage(SourceImage, 0))
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Source texture mip is unavailable."));
            return 7;
        }
        const int32 SourceChannels = ERawImageFormat::NumChannels(SourceImage.Format);
        if (SourceChannels != 3 && SourceChannels != 4)
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Unsupported source texture channel count: %d."), SourceChannels);
            return 7;
        }
        const bool bUsesPng = SourceChannels == 3 || SourceImage.Format == ERawImageFormat::RGBA16;
        const FString ImagePath = FPaths::ChangeExtension(OutputPath, bUsesPng ? TEXT("png") : TEXT("tga"));
        bool bExported = false;
        if (SourceChannels == 4 && SourceImage.Format == ERawImageFormat::BGRA8
            && SourceImage.NumSlices == 1 && SourceImage.SizeX <= MAX_uint16 && SourceImage.SizeY <= MAX_uint16)
        {
            TArray64<uint8> EncodedSource;
            EncodedSource.SetNumZeroed(18 + SourceImage.RawData.Num());
            EncodedSource[2] = 2;
            EncodedSource[12] = SourceImage.SizeX & 255;
            EncodedSource[13] = SourceImage.SizeX >> 8;
            EncodedSource[14] = SourceImage.SizeY & 255;
            EncodedSource[15] = SourceImage.SizeY >> 8;
            EncodedSource[16] = 32;
            EncodedSource[17] = 8;
            const int64 RowBytes = static_cast<int64>(SourceImage.SizeX) * 4;
            for (int32 Row = 0; Row < SourceImage.SizeY; ++Row)
            {
                FMemory::Memcpy(EncodedSource.GetData() + 18 + Row * RowBytes,
                    SourceImage.RawData.GetData() + (SourceImage.SizeY - 1 - Row) * RowBytes, RowBytes);
            }
            bExported = FFileHelper::SaveArrayToFile(EncodedSource, *ImagePath);
        }
        else if (bUsesPng)
        {
            bExported = FImageUtils::SaveImageByExtension(*ImagePath, SourceImage);
        }
        if (!bExported)
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Unable to export source texture: format=%s channels=%d."), ERawImageFormat::GetName(SourceImage.Format), SourceChannels);
            return 7;
        }
        FImage ExportedImage;
        TArray<uint8> EncodedImage;
        if (!FImageUtils::LoadImage(*ImagePath, ExportedImage)
            || !FFileHelper::LoadFileToArray(EncodedImage, *ImagePath) || EncodedImage.Num() < 26
            || (!bUsesPng && (EncodedImage[16] != 32 || (EncodedImage[17] & 15) != 8))
            || (bUsesPng && EncodedImage[25] != (SourceChannels == 4 ? 6 : 2))
            || (SourceImage.Format == ERawImageFormat::RGBA16 && EncodedImage[24] != 16)
            || SourceImage.SizeX != ExportedImage.SizeX || SourceImage.SizeY != ExportedImage.SizeY
            || SourceImage.NumSlices != ExportedImage.NumSlices || SourceImage.Format != ExportedImage.Format
            || SourceImage.RawData != ExportedImage.RawData)
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Exported texture differs from its source mip pixels or format."));
            return 8;
        }
        Root->SetStringField(TEXT("source_image"), FPaths::GetCleanFilename(ImagePath));
        Root->SetNumberField(TEXT("source_width"), Texture->Source.GetSizeX());
        Root->SetNumberField(TEXT("source_height"), Texture->Source.GetSizeY());
        Root->SetStringField(TEXT("source_pixel_format"), ERawImageFormat::GetName(SourceImage.Format));
        Root->SetNumberField(TEXT("source_channels"), SourceChannels);
        Root->SetNumberField(TEXT("exported_channels"), SourceChannels);
        Root->SetBoolField(TEXT("source_image_pixel_exact"), true);
    }
    TSharedRef<FJsonObject> DebugFonts = MakeShared<FJsonObject>();
    FSlateFontCache DebugFontCache(FModuleManager::LoadModuleChecked<ISlateNullRendererModule>(TEXT("SlateNullRenderer")).CreateSlateFontAtlasFactory(), ESlateTextureAtlasThreadId::Game);
    const UFont* Fonts[] = {UEngine::GetTinyFont(), UEngine::GetMediumFont()};
    const TCHAR* FontNames[] = {TEXT("tiny"), TEXT("medium")};
    for (int32 Index = 0; Index < 2; ++Index)
    {
        if (IsValid(Fonts[Index]))
        {
            TSharedRef<FJsonObject> FontValue = MakeShared<FJsonObject>();
            FontValue->SetNumberField(TEXT("size_points"), Fonts[Index]->LegacyFontSize);
            FontValue->SetNumberField(TEXT("max_char_height"), DebugFontCache.GetCharacterList(FSlateFontInfo(Fonts[Index], Fonts[Index]->LegacyFontSize), 1.0f).GetMaxHeight());
            FontValue->SetStringField(TEXT("style"), Fonts[Index]->LegacyFontName.ToString());
            DebugFonts->SetObjectField(FontNames[Index], FontValue);
        }
    }
    Root->SetObjectField(TEXT("engine_debug_fonts"), DebugFonts);
    Root->SetStringField(TEXT("blueprint_cdo_path"), TEXT("None"));
    Root->SetArrayField(TEXT("blueprint_cdo_properties"), {});
    TArray<TSharedPtr<FJsonValue>> DefaultSubobjectValues;
    if (Blueprint != nullptr && Blueprint->GeneratedClass != nullptr)
    {
        UObject* DefaultObject = Blueprint->GeneratedClass->GetDefaultObject();
        if (IsValid(DefaultObject))
        {
            Root->SetStringField(TEXT("blueprint_cdo_path"), ObjectPath(DefaultObject));
            Root->SetArrayField(
                TEXT("blueprint_cdo_properties"),
                CaptureProperties(DefaultObject->GetClass(), DefaultObject, DefaultObject)
            );
            TArray<UObject*> DefaultSubobjects;
            GetObjectsWithOuter(DefaultObject, DefaultSubobjects, EGetObjectsFlags::IncludeNestedObjects);
            DefaultSubobjects.Sort([](const UObject& Left, const UObject& Right)
            {
                return Left.GetPathName() < Right.GetPathName();
            });
            for (int32 ObjectIndex = 0; ObjectIndex < DefaultSubobjects.Num(); ++ObjectIndex)
            {
                DefaultSubobjectValues.Add(MakeShared<FJsonValueObject>(CaptureObject(DefaultSubobjects[ObjectIndex], ObjectIndex)));
            }
        }
    }
    Root->SetArrayField(TEXT("blueprint_cdo_subobjects"), DefaultSubobjectValues);

    TArray<TSharedPtr<FJsonValue>> AssetObjectValues;
    for (int32 ObjectIndex = 0; ObjectIndex < AssetObjects.Num(); ++ObjectIndex)
    {
        AssetObjectValues.Add(MakeShared<FJsonValueObject>(CaptureObject(AssetObjects[ObjectIndex], ObjectIndex)));
    }
    Root->SetArrayField(TEXT("asset_objects"), AssetObjectValues);

    TArray<TSharedPtr<FJsonValue>> RigUnitDefaults;
    for (UObject* Object : AssetObjects)
    {
        const URigVMUnitNode* Unit = Cast<URigVMUnitNode>(Object);
        if (Unit == nullptr)
        {
            continue;
        }
        const UScriptStruct* Struct = Unit->GetScriptStruct();
        const TSharedPtr<FStructOnScope> Constructor = Unit->ConstructStructInstance(true);
        const TSharedPtr<FStructOnScope> Authored = Unit->ConstructStructInstance(false);
        if (Struct == nullptr || !Constructor.IsValid() || !Authored.IsValid())
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Rig unit defaults are unavailable: %s."), *Unit->GetPathName());
            return 6;
        }
        TSharedRef<FJsonObject> Entry = MakeShared<FJsonObject>();
        Entry->SetStringField(TEXT("object_path"), Unit->GetPathName());
        Entry->SetStringField(TEXT("script_struct"), Struct->GetPathName());
        Entry->SetStringField(TEXT("method"), Unit->GetMethodName().ToString());
        Entry->SetStringField(TEXT("event"), Unit->GetEventName().ToString());
        Entry->SetBoolField(TEXT("pure"), Unit->IsPure());
        Entry->SetBoolField(TEXT("constant"), Unit->IsDefinedAsConstant());
        Entry->SetBoolField(TEXT("varying"), Unit->IsDefinedAsVarying());
        TArray<TSharedPtr<FJsonValue>> Blocks;
        for (const FName Block : Unit->GetControlFlowBlocks())
        {
            Blocks.Add(MakeShared<FJsonValueString>(Block.ToString()));
        }
        Entry->SetArrayField(TEXT("control_flow_blocks"), Blocks);
        Entry->SetArrayField(TEXT("constructor_properties"), CaptureProperties(Struct, Constructor->GetStructMemory(), Object, true));
        Entry->SetArrayField(TEXT("authored_properties"), CaptureProperties(Struct, Authored->GetStructMemory(), Object, true));
        RigUnitDefaults.Add(MakeShared<FJsonValueObject>(Entry));
    }
    if (!RigUnitDefaults.IsEmpty())
    {
        Root->SetArrayField(TEXT("rig_unit_defaults"), RigUnitDefaults);
        UControlRig* Rig = Blueprint != nullptr && Blueprint->GeneratedClass != nullptr
            ? Cast<UControlRig>(Blueprint->GeneratedClass->GetDefaultObject()) : nullptr;
        const FObjectPropertyBase* VMProperty = Rig != nullptr ? FindFProperty<FObjectPropertyBase>(Rig->GetClass(), TEXT("VM")) : nullptr;
        const URigVM* VM = VMProperty != nullptr ? Cast<URigVM>(VMProperty->GetObjectPropertyValue_InContainer(Rig)) : nullptr;
        if (VM == nullptr)
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Compiled RigVM is unavailable."));
            return 7;
        }
        Root->SetObjectField(TEXT("rig_vm"), CaptureRigVM(*VM, Rig));
        if (Rig->GetHierarchy() == nullptr)
        {
            UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Compiled Rig hierarchy is unavailable."));
            return 7;
        }
        Root->SetObjectField(TEXT("rig_cdo_hierarchy"), CaptureRigHierarchy(Rig->GetHierarchy()));
    }

    TArray<TSharedPtr<FJsonValue>> GraphValues;
    for (int32 GraphIndex = 0; GraphIndex < Graphs.Num(); ++GraphIndex)
    {
        GraphValues.Add(MakeShared<FJsonValueObject>(CaptureGraph(Graphs[GraphIndex], GraphIndex)));
    }
    Root->SetArrayField(TEXT("graphs"), GraphValues);
    if (const FObjectPropertyBase* HierarchyProperty = FindFProperty<FObjectPropertyBase>(Asset->GetClass(), TEXT("Hierarchy")))
    {
        if (const URigHierarchy* Hierarchy = Cast<URigHierarchy>(HierarchyProperty->GetObjectPropertyValue_InContainer(Asset)))
        {
            Root->SetObjectField(TEXT("rig_hierarchy"), CaptureRigHierarchy(Hierarchy));
        }
    }
    if (const UPhysicsAsset* PhysicsAsset = Cast<UPhysicsAsset>(Asset))
    {
        Root->SetObjectField(TEXT("physics_asset"), CapturePhysicsAsset(PhysicsAsset));
    }
    if (const USkeletalMesh* SkeletalMesh = Cast<USkeletalMesh>(Asset))
    {
        Root->SetObjectField(TEXT("skeletal_mesh"), CaptureSkeletalMesh(SkeletalMesh));
    }

    if (!BlendSpaceProbePath.IsEmpty())
    {
        const TSharedPtr<FJsonObject> Probe = CaptureBlendSpaceProbe(Asset, BlendSpaceProbePath);
        if (!Probe.IsValid())
        {
            return 7;
        }
        Root->SetObjectField(TEXT("blendspace_probe"), Probe.ToSharedRef());
    }
    FString RigLifecycleProbePath;
    if (FParse::Value(*Params, TEXT("RigLifecycleProbe="), RigLifecycleProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureRigLifecycleProbe(Asset, RigLifecycleProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("rig_lifecycle_probe"), Probe.ToSharedRef());
    }
    FString AnimationAttributeProbePath;
    if (FParse::Value(*Params, TEXT("AnimationAttributeProbe="), AnimationAttributeProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureAnimationAttributeProbe(AnimationAttributeProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("animation_attribute_probe"), Probe.ToSharedRef());
    }
    FString CharacterAnimationProbePath;
    if (FParse::Value(*Params, TEXT("CharacterAnimationProbe="), CharacterAnimationProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureCharacterAnimationProbe(Asset, CharacterAnimationProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("character_animation_probe"), Probe.ToSharedRef());
    }
    FString RigNodeHistoryProbePath;
    if (FParse::Value(*Params, TEXT("RigNodeHistoryProbe="), RigNodeHistoryProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureRigNodeHistoryProbe(Asset, RigNodeHistoryProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("rig_node_history_probe"), Probe.ToSharedRef());
    }
    FString RigPoseTransferProbePath;
    if (FParse::Value(*Params, TEXT("RigPoseTransferProbe="), RigPoseTransferProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureRigPoseTransferProbe(Asset, RigPoseTransferProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("rig_pose_transfer_probe"), Probe.ToSharedRef());
    }
    if (!SequencePoseProbePath.IsEmpty())
    {
        const TSharedPtr<FJsonObject> Probe = CaptureSequencePoseProbe(Asset, SequencePoseProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("sequence_pose_probe"), Probe.ToSharedRef());
    }
    FString MontageClockProbePath;
    if (FParse::Value(*Params, TEXT("MontageClockProbe="), MontageClockProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureMontageClockProbe(Asset, MontageClockProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("montage_clock_probe"), Probe.ToSharedRef());
    }
    FString SequenceClockProbePath;
    if (FParse::Value(*Params, TEXT("SequenceClockProbe="), SequenceClockProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureSequenceClockProbe(Asset, SequenceClockProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("sequence_clock_probe"), Probe.ToSharedRef());
    }
    FString BlendProfileProbePath;
    FString BlendListProbePath;
    if (FParse::Value(*Params, TEXT("BlendListProbe="), BlendListProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureBlendListProbe(Asset, BlendListProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("blend_list_probe"), Probe.ToSharedRef());
    }
    FString SyncGroupClockProbePath;
    if (FParse::Value(*Params, TEXT("SyncGroupClockProbe="), SyncGroupClockProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureSyncGroupClockProbe(Asset, SyncGroupClockProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("sync_group_clock_probe"), Probe.ToSharedRef());
    }
    if (FParse::Value(*Params, TEXT("BlendProfileProbe="), BlendProfileProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureBlendProfileProbe(Asset, BlendProfileProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("blend_profile_probe"), Probe.ToSharedRef());
    }
    FString SlotProbePath;
    if (FParse::Value(*Params, TEXT("SlotProbe="), SlotProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureSlotProbe(Asset, SlotProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("slot_probe"), Probe.ToSharedRef());
    }
    FString MeshAdditiveProbePath;
    if (FParse::Value(*Params, TEXT("MeshAdditiveProbe="), MeshAdditiveProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureMeshAdditiveProbe(Asset, MeshAdditiveProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("mesh_additive_probe"), Probe.ToSharedRef());
    }
    FString LayeredBoneBlendProbePath;
    if (FParse::Value(*Params, TEXT("LayeredBoneBlendProbe="), LayeredBoneBlendProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureLayeredBoneBlendProbe(Asset, LayeredBoneBlendProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("layered_bone_blend_probe"), Probe.ToSharedRef());
    }
    FString InertializationProbePath;
    if (FParse::Value(*Params, TEXT("InertializationProbe="), InertializationProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureInertializationProbe(Asset, InertializationProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("inertialization_probe"), Probe.ToSharedRef());
    }
    if (FParse::Param(*Params, TEXT("CachedPoseSchedule")))
    {
        const TSharedPtr<FJsonObject> Schedule = CaptureCachedPoseSchedule(Blueprint);
        if (!Schedule.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("cached_pose_schedule"), Schedule.ToSharedRef());
    }
    FString StateMachineEntryProbePath;
    if (FParse::Value(*Params, TEXT("StateMachineEntryProbe="), StateMachineEntryProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureStateMachineEntryProbe(Asset, StateMachineEntryProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("state_machine_entry_probe"), Probe.ToSharedRef());
    }
    if (FParse::Param(*Params, TEXT("StateMachinePlayers")))
    {
        const TSharedPtr<FJsonObject> Players = CaptureStateMachinePlayers(Asset);
        if (!Players.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("state_machine_players"), Players.ToSharedRef());
    }
    if (FParse::Param(*Params, TEXT("CachedPoseLifecycleProbe")))
    {
        Root->SetObjectField(TEXT("cached_pose_lifecycle_probe"), CaptureCachedPoseLifecycleProbe());
    }
    if (FParse::Param(*Params, TEXT("AlphaBlendProbe")))
    {
        Root->SetObjectField(TEXT("alpha_blend_probe"), CaptureAlphaBlendProbe());
    }
    if (FParse::Param(*Params, TEXT("NotifyClockProbe")))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureNotifyClockProbe(Asset);
        if (!Probe.IsValid())
        {
            return 7;
        }
        Root->SetObjectField(TEXT("notify_clock_probe"), Probe.ToSharedRef());
    }
    if (FParse::Param(*Params, TEXT("NotifyDispatchProbe")))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureNotifyDispatchProbe(Asset);
        if (!Probe.IsValid()) { return 9; }
        Root->SetObjectField(TEXT("notify_dispatch_probe"), Probe.ToSharedRef());
    }
    if (FParse::Param(*Params, TEXT("NotifyChanceProbe")))
    {
        Root->SetObjectField(TEXT("notify_chance_probe"), CaptureNotifyChanceProbe(Asset));
    }
    if (FParse::Param(*Params, TEXT("PhysicsMaterialProbe")))
    {
        Root->SetObjectField(TEXT("physics_material_probe"), CapturePhysicsMaterialProbe());
    }
    FString PhysicsBodyProbePath;
    if (FParse::Value(*Params, TEXT("PhysicsBodyProbe="), PhysicsBodyProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CapturePhysicsBodyProbe(Asset, PhysicsBodyProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("physics_body_probe"), Probe.ToSharedRef());
    }
    FString PhysicsContactProbePath;
    if (FParse::Value(*Params, TEXT("PhysicsContactProbe="), PhysicsContactProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CapturePhysicsContactProbe(Asset, PhysicsContactProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("physics_contact_probe"), Probe.ToSharedRef());
    }
    FString GeometryQueryProbePath;
    if (FParse::Value(*Params, TEXT("GeometryQueryProbe="), GeometryQueryProbePath))
    {
        const TSharedPtr<FJsonObject> Probe = CaptureGeometryQueryProbe(GeometryQueryProbePath);
        if (!Probe.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("geometry_query_probe"), Probe.ToSharedRef());
    }

    FString Json;
    const TSharedRef<TJsonWriter<TCHAR, TPrettyJsonPrintPolicy<TCHAR>>> Writer = TJsonWriterFactory<TCHAR, TPrettyJsonPrintPolicy<TCHAR>>::Create(&Json);
    if (!FJsonSerializer::Serialize(Root, Writer))
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Unable to serialize graph export."));
        return 4;
    }

    const FString Directory = FPaths::GetPath(OutputPath);
    if (!IFileManager::Get().MakeDirectory(*Directory, true))
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Unable to create output directory."));
        return 5;
    }

    if (!FFileHelper::SaveStringToFile(Json, *OutputPath, FFileHelper::EEncodingOptions::ForceUTF8WithoutBOM))
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Unable to write graph export."));
        return 6;
    }

    UE_LOG(LogAlsGraphSourceReader, Display, TEXT("Exported %d graphs and %d asset objects to %s."), Graphs.Num(), AssetObjects.Num(), *OutputPath);
    return 0;
}
