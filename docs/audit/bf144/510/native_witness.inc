        if ((*Case)->HasField(TEXT("acceleration_witness")))
        {
            const TSharedPtr<FJsonObject>* RequestWitness = nullptr;
            if (!(*Case)->TryGetObjectField(TEXT("acceleration_witness"), RequestWitness)) { return nullptr; }
            auto ReadArray = [&](const TCHAR* Key, TArray<double>& Values, int32 Count)
            {
                const TArray<TSharedPtr<FJsonValue>>* Input = nullptr;
                if (!(*RequestWitness)->TryGetArrayField(Key, Input) || Input->Num() != Count) { return false; }
                for (const auto& Element : *Input)
                {
                    double Value;
                    if (Element->Type != EJson::Number || !Element->TryGetNumber(Value) || !FMath::IsFinite(Value)) { return false; }
                    Values.Add(Value);
                }
                return true;
            };
            TArray<double> Acceleration, Velocity, Rotation;
            double MaxAcceleration = 0, MaxBraking = 0;
            bool bCall = false;
            if (!ReadArray(TEXT("acceleration"), Acceleration, 3) ||
                !ReadArray(TEXT("velocity"), Velocity, 3) ||
                !ReadArray(TEXT("rotation"), Rotation, 4) ||
                !(*RequestWitness)->TryGetNumberField(TEXT("max_acceleration"), MaxAcceleration) ||
                !(*RequestWitness)->TryGetNumberField(TEXT("max_braking"), MaxBraking) ||
                !(*RequestWitness)->TryGetBoolField(TEXT("call_refresh"), bCall)) { return nullptr; }
            const FStructProperty* Locomotion = FindFProperty<FStructProperty>(Instance->GetClass(), TEXT("LocomotionState"));
            if (Locomotion == nullptr) { return nullptr; }
            const FStructProperty* AccelerationField = FindFProperty<FStructProperty>(Locomotion->Struct, TEXT("AccelerationWorldSpace"));
            const FStructProperty* VelocityField = FindFProperty<FStructProperty>(Locomotion->Struct, TEXT("VelocityWorldSpace"));
            const FStructProperty* RotationField = FindFProperty<FStructProperty>(Locomotion->Struct, TEXT("RotationQuaternionWorldSpace"));
            const FFloatProperty* AccelerationLimit = FindFProperty<FFloatProperty>(Locomotion->Struct, TEXT("MaxAcceleration"));
            const FFloatProperty* BrakingLimit = FindFProperty<FFloatProperty>(Locomotion->Struct, TEXT("MaxBrakingDeceleration"));
            UFunction* Refresh = Instance->FindFunction(TEXT("RefreshGrounded"));
            if (AccelerationField == nullptr || AccelerationField->Struct != TBaseStructure<FVector>::Get() ||
                VelocityField == nullptr || VelocityField->Struct != TBaseStructure<FVector>::Get() ||
                RotationField == nullptr || RotationField->Struct != TBaseStructure<FQuat>::Get() ||
                AccelerationLimit == nullptr || BrakingLimit == nullptr || Refresh == nullptr || Refresh->ParmsSize != 0) { return nullptr; }
            void* State = Locomotion->ContainerPtrToValuePtr<void>(Instance);
            *AccelerationField->ContainerPtrToValuePtr<FVector>(State) = FVector(Acceleration[0], Acceleration[1], Acceleration[2]);
            *VelocityField->ContainerPtrToValuePtr<FVector>(State) = FVector(Velocity[0], Velocity[1], Velocity[2]);
            *RotationField->ContainerPtrToValuePtr<FQuat>(State) = FQuat(Rotation[0], Rotation[1], Rotation[2], Rotation[3]);
            AccelerationLimit->SetPropertyValue_InContainer(State, static_cast<float>(MaxAcceleration));
            BrakingLimit->SetPropertyValue_InContainer(State, static_cast<float>(MaxBraking));
            if (bCall) { Instance->ProcessEvent(Refresh, nullptr); }
            CaseRow->SetArrayField(TEXT("acceleration_witness"), CaptureState(Instance));
        }
