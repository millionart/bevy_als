#include "AlsGraphSourceReaderCommandlet.h"

#include "Dom/JsonObject.h"
#include "EdGraph/EdGraph.h"
#include "EdGraph/EdGraphNode.h"
#include "EdGraph/EdGraphPin.h"
#include "AnimStateAliasNode.h"
#include "AnimStateNodeBase.h"
#include "AnimStateTransitionNode.h"
#include "AnimationRuntime.h"
#include "AlphaBlend.h"
#include "Animation/AnimInstanceProxy.h"
#include "Animation/AnimInstance.h"
#include "Components/SkeletalMeshComponent.h"
#include "Animation/AnimClassInterface.h"
#include "Animation/AnimNode_SaveCachedPose.h"
#include "Animation/AnimNotifyQueue.h"
#include "Animation/AnimNotifies/AnimNotifyState.h"
#include "Animation/AnimSequence.h"
#include "Animation/AnimationPoseData.h"
#include "Animation/AnimMontage.h"
#include "Animation/BlendSpace.h"
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
#include "Chaos/Sphere.h"
#include "Chaos/GeometryQueries.h"
#include "Chaos/ChaosEngineInterface.h"
#include "PBDRigidsSolver.h"
#include "ChaosSolversModule.h"
#include "Chaos/CollisionResolution.h"
#include "Chaos/Collision/CollisionConstraintAllocator.h"
#include "Chaos/Collision/CollisionFilter.h"
#include "PhysicsEngine/BodySetup.h"
#include "PhysicsEngine/PhysicsConstraintTemplate.h"
#include "PhysicsEngine/SkeletalBodySetup.h"
#include "PhysicalMaterials/PhysicalMaterial.h"
#include "Rigs/RigHierarchy.h"
#include "ControlRig.h"
#include "RigVMCore/RigVM.h"
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
    TSharedRef<FJsonObject> ConsoleVariables = MakeShared<FJsonObject>();
    const FConsoleObjectVisitor Visitor = FConsoleObjectVisitor::CreateLambda([&ConsoleVariables](const TCHAR* Name, IConsoleObject* Object)
    {
        if (IConsoleVariable* Variable = Object->AsVariable())
        {
            ConsoleVariables->SetStringField(Name, Variable->GetString());
        }
    });
    for (const TCHAR* Prefix : {TEXT("p.Chaos.JointConstraint."), TEXT("p.Chaos.Solver.Joint."), TEXT("p.Chaos.Joint."), TEXT("p.Chaos.CCD."), TEXT("p.Chaos.Solver.InertiaConditioning."), TEXT("p.Chaos.InertiaConditioning."), TEXT("p.Chaos.ExcludeFreeJointForInertiaConditioning"), TEXT("p.Chaos.Solver.Iterations."), TEXT("p.Chaos.Solver.ComputeIterationSettings."), TEXT("p.AngularEtherDragOverride"), TEXT("p.HackMaxAngularVelocity"), TEXT("p.LinearEtherDragOverride"), TEXT("p.HackMaxVelocity2"), TEXT("p.Chaos.Solver.Sleep."), TEXT("p.Chaos.SmoothedPositionLerpRate"), TEXT("p.Chaos.Solver.Collision."), TEXT("p.CollisionRestitution"), TEXT("p.Chaos.PBDCollisionSolver."), TEXT("p.Chaos.Solver.bChaosForceMACD"), TEXT("p.Chaos.Collision.Stiffness"), TEXT("p.CollisionFriction"), TEXT("p.CollisionAngularFriction"), TEXT("p.CollisionBaseFrictionImpulse"), TEXT("p.Chaos.Solver.Use1DFriction"), TEXT("p.Chaos.Collision.Manifold.Friction"), TEXT("p.Chaos.Collision.Manifold.EnableFrictionRestore"), TEXT("p.Chaos.Collision.SimpleAssignContact")})
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
        const bool bCapsule = (*Input)->HasField(TEXT("capsule_sweep"));
        const TCHAR* SweepKey = bCapsule ? TEXT("capsule_sweep") : TEXT("sphere_sweep");
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
                || !(*Sweep)->TryGetNumberField(TEXT("radius_cm"), Radius) || !FMath::IsFinite(Radius) || Radius <= 0
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
            FPhysicsShapeAdapter QueryAdapter(FQuat::Identity, bCapsule
                ? FCollisionShape::MakeCapsule(Radius, HalfHeight) : FCollisionShape::MakeSphere(Radius));
            Chaos::FReal Time = Length;
            Chaos::FVec3 Position = Chaos::FVec3::ZeroVector;
            Chaos::FVec3 Normal = Chaos::FVec3::ZeroVector;
            Chaos::FVec3 FaceNormal = Chaos::FVec3::ZeroVector;
            int32 FaceIndex = INDEX_NONE;
            const bool Hit = Chaos::SweepQuery(BoxAdapter.GetGeometry(), FTransform(Rotation, BoxTranslation),
                QueryAdapter.GetGeometry(), QueryAdapter.GetGeomPose(Start), Direction, Length, Time, Position, Normal,
                FaceIndex, FaceNormal, 0.0, true);
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
        Chaos::Private::FCollisionContextAllocator Allocator(nullptr, 1, 0);
        auto Constraint = Allocator.CreateConstraint(Particle0, Implicit, Particle0->ShapeInstances()[0].Get(), nullptr,
            Chaos::FRigidTransform3::Identity, Particle1, Other, Particle1->ShapeInstances()[0].Get(), nullptr,
            Chaos::FRigidTransform3::Identity, CullDistance, true, PairType);
        if (!Constraint.IsValid()) { return nullptr; }
        const FTransform ShapeWorld = ShapeLocal * FTransform(BodyRotation, BodyTranslation);
        const FTransform BoxWorld = OtherLocal * FTransform(OtherRotation, OtherTranslation);
        Constraint->SetShapeWorldTransforms(ShapeWorld, BoxWorld);
        Chaos::Collisions::UpdateConstraint(*Constraint, ShapeWorld, BoxWorld, DeltaSeconds);
        const auto CapturePoints = [&Constraint](const FTransform& ShapeTransform, const FTransform& BoxTransform)
        {
            TArray<TSharedPtr<FJsonValue>> Points;
            for (const Chaos::FManifoldPoint& Point : Constraint->GetManifoldPoints())
            {
                if (Point.Flags.bDisabled) { continue; }
                const auto& Contact = Point.ContactPoint;
                TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
                Value->SetNumberField(TEXT("phi_cm"), Contact.Phi);
                Value->SetObjectField(TEXT("shape_point_0_cm"), CaptureVector(FVector(Contact.ShapeContactPoints[0])));
                Value->SetObjectField(TEXT("shape_point_1_cm"), CaptureVector(FVector(Contact.ShapeContactPoints[1])));
                Value->SetObjectField(TEXT("world_point_0_cm"), CaptureVector(ShapeTransform.TransformPositionNoScale(FVector(Contact.ShapeContactPoints[0]))));
                Value->SetObjectField(TEXT("world_point_1_cm"), CaptureVector(BoxTransform.TransformPositionNoScale(FVector(Contact.ShapeContactPoints[1]))));
                Value->SetObjectField(TEXT("world_normal"), CaptureVector(BoxTransform.TransformVectorNoScale(FVector(Contact.ShapeContactNormal))));
                Points.Add(MakeShared<FJsonValueObject>(Value));
            }
            return Points;
        };
        const TArray<TSharedPtr<FJsonValue>> Points = CapturePoints(ShapeWorld, BoxWorld);
        TSharedRef<FJsonObject> Sample = MakeShared<FJsonObject>();
        Sample->SetObjectField(TEXT("input"), Input.ToSharedRef());
        Sample->SetObjectField(TEXT("geometry"), GeometryValue);
        if (OtherGeometryValue.IsValid()) { Sample->SetObjectField(TEXT("other_geometry"), OtherGeometryValue.ToSharedRef()); }
        Sample->SetArrayField(TEXT("points"), Points);
        if (HasNext)
        {
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
            Sample->SetArrayField(TEXT("next_points"), CapturePoints(NextShapeWorld, NextBoxWorld));
            Constraint->ResetActiveManifoldContacts();
            Chaos::Collisions::UpdateConstraint(*Constraint, NextShapeWorld, NextBoxWorld, DeltaSeconds);
            Sample->SetArrayField(TEXT("fresh_points"), CapturePoints(NextShapeWorld, NextBoxWorld));
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
    bool bRootTrackOnly = false;
    Request->TryGetBoolField(TEXT("root_track_only"), bRootTrackOnly);
    bool bAnimationPose = false;
    Request->TryGetBoolField(TEXT("animation_pose"), bAnimationPose);
    if (bRootTrackOnly && bAnimationPose)
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Sequence pose sampling modes are mutually exclusive."));
        return nullptr;
    }
    const FReferenceSkeleton& Skeleton = Sequence->GetSkeleton()->GetReferenceSkeleton();
    TArray<FBoneIndexType> BoneIndices;
    FBoneContainer RequiredBones;
    if (bAnimationPose)
    {
        for (int32 Index = 0; Index < Skeleton.GetNum(); ++Index)
        {
            BoneIndices.Add(Index);
        }
        RequiredBones.InitializeTo(BoneIndices, UE::Anim::FCurveFilterSettings(), *Sequence->GetSkeleton());
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
        if (bAnimationPose)
        {
            Pose.SetBoneContainer(&RequiredBones);
            Curve.InitFrom(RequiredBones);
            FAnimationPoseData PoseData(Pose, Curve, Attributes);
            Sequence->GetAnimationPose(PoseData, FAnimExtractContext(Time, false));
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
                Sequence->GetBoneTransform(Transform, FSkeletonPoseBoneIndex(Index), FAnimExtractContext(Time), true);
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
            Bones.Add(MakeShared<FJsonValueObject>(Bone));
        }
        Frame->SetArrayField(TEXT("bones"), Bones);
        Frames.Add(MakeShared<FJsonValueObject>(Frame));
    }
    TSharedRef<FJsonObject> Probe = MakeShared<FJsonObject>();
    Probe->SetStringField(TEXT("sampling_mode"), bAnimationPose ? TEXT("animation_pose") : bRootTrackOnly ? TEXT("root_track") : TEXT("raw_local_tracks"));
    Probe->SetNumberField(TEXT("duration"), Sequence->GetPlayLength());
    Probe->SetArrayField(TEXT("frames"), Frames);
    return Probe;
}

TSharedPtr<FJsonObject> CaptureMontageClockProbe(UObject* Asset, const FString& RequestPath)
{
    UAnimMontage* Montage = Cast<UAnimMontage>(Asset);
    FString Text;
    TSharedPtr<FJsonObject> Request;
    const TArray<TSharedPtr<FJsonValue>>* Samples = nullptr;
    if (Montage == nullptr || !FFileHelper::LoadFileToString(Text, *RequestPath) ||
        !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Request) ||
        !Request->TryGetArrayField(TEXT("samples"), Samples) || Samples->IsEmpty())
    {
        UE_LOG(LogAlsGraphSourceReader, Error, TEXT("Montage clock probe asset or request is invalid."));
        return nullptr;
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
        Instance.Play();
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
                if (Instance.IsValid()) { Instance.Stop(FAlphaBlend(Montage->BlendOut, StopDuration)); }
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
        virtual void Initialize_AnyThread(const FAnimationInitializeContext& Context) override
        {
            ++Initializations;
            Position = 0.2f;
        }
        virtual void Update_AnyThread(const FAnimationUpdateContext& Context) override
        {
            ++Updates;
            Position += Context.GetDeltaTime();
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
    if (FParse::Param(*Params, TEXT("CachedPoseSchedule")))
    {
        const TSharedPtr<FJsonObject> Schedule = CaptureCachedPoseSchedule(Blueprint);
        if (!Schedule.IsValid()) { return 7; }
        Root->SetObjectField(TEXT("cached_pose_schedule"), Schedule.ToSharedRef());
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
