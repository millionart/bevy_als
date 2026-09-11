#include "AlsGraphSourceReaderCommandlet.h"

#include "Dom/JsonObject.h"
#include "EdGraph/EdGraph.h"
#include "EdGraph/EdGraphNode.h"
#include "EdGraph/EdGraphPin.h"
#include "AnimStateAliasNode.h"
#include "AnimStateNodeBase.h"
#include "AnimStateTransitionNode.h"
#include "AnimationRuntime.h"
#include "Animation/AnimInstanceProxy.h"
#include "Animation/AnimNode_SaveCachedPose.h"
#include "Animation/AnimNotifyQueue.h"
#include "Animation/AnimSequence.h"
#include "Animation/BlendSpace.h"
#include "Animation/Skeleton.h"
#include "Engine/Blueprint.h"
#include "Engine/SkeletalMesh.h"
#include "PhysicsEngine/AggregateGeom.h"
#include "PhysicsEngine/PhysicsAsset.h"
#include "PhysicsEngine/PhysicsConstraintTemplate.h"
#include "PhysicsEngine/SkeletalBodySetup.h"
#include "PhysicalMaterials/PhysicalMaterial.h"
#include "Rigs/RigHierarchy.h"
#include "RigVMModel/Nodes/RigVMUnitNode.h"
#include "HAL/FileManager.h"
#include "HAL/IConsoleManager.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "Serialization/JsonSerializer.h"
#include "Serialization/JsonWriter.h"
#include "UObject/UObjectGlobals.h"
#include "UObject/UObjectHash.h"
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
            Values.Add(MakeShared<FJsonValueObject>(Entry));
        }
    }

    return Values;
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
    Value->SetNumberField(TEXT("shape_count"), Body->AggGeom.GetElementCount());
    if (UPhysicalMaterial* Material = Body->GetPhysMaterial())
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
    TSharedRef<FJsonObject> ConsoleVariables = MakeShared<FJsonObject>();
    const FConsoleObjectVisitor Visitor = FConsoleObjectVisitor::CreateLambda([&ConsoleVariables](const TCHAR* Name, IConsoleObject* Object)
    {
        if (IConsoleVariable* Variable = Object->AsVariable())
        {
            ConsoleVariables->SetStringField(Name, Variable->GetString());
        }
    });
    for (const TCHAR* Prefix : {TEXT("p.Chaos.JointConstraint."), TEXT("p.Chaos.Solver.Joint."), TEXT("p.Chaos.Joint.")})
    {
        IConsoleManager::Get().ForEachConsoleObjectThatStartsWith(Visitor, Prefix);
    }
    Value->SetObjectField(TEXT("constraint_console_variables"), ConsoleVariables);
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

TSharedRef<FJsonObject> CaptureRigHierarchy(const URigHierarchy* Hierarchy)
{
    TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
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
    FParse::Value(*Params, TEXT("Asset="), AssetPath);
    FParse::Value(*Params, TEXT("Output="), OutputPath);
    FParse::Value(*Params, TEXT("BlendSpaceProbe="), BlendSpaceProbePath);

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
    if (FParse::Param(*Params, TEXT("CachedPoseLifecycleProbe")))
    {
        Root->SetObjectField(TEXT("cached_pose_lifecycle_probe"), CaptureCachedPoseLifecycleProbe());
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
