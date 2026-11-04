        if ((*Case)->HasField(TEXT("ramp_pitch")))
        {
            double Pitch;
            bool bCanStep;
            if (!(*Case)->TryGetNumberField(TEXT("ramp_pitch"), Pitch) ||
                !(*Case)->TryGetBoolField(TEXT("ramp_step"), bCanStep) ||
                (Pitch != 40 && Pitch != 60)) { return nullptr; }
            const auto SpawnBox = [&](const TCHAR* Name, const FVector& Center, const FRotator& Rotation,
                                      const FVector& Extent, bool bStep)
            {
                AActor* Actor = World->SpawnActor<AActor>();
                if (Actor == nullptr) { return false; }
                UBoxComponent* Box = NewObject<UBoxComponent>(Actor, FName(Name));
                Actor->SetRootComponent(Box);
                Actor->AddInstanceComponent(Box);
                Box->SetMobility(EComponentMobility::Static);
                Box->SetBoxExtent(Extent);
                Box->SetWorldLocationAndRotation(Center, Rotation);
                Box->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
                Box->SetCollisionResponseToAllChannels(ECR_Block);
                Box->CanCharacterStepUpOn = bStep ? ECB_Yes : ECB_No;
                Box->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
                Box->RegisterComponent();
                return true;
            };
            const FRotator RampRotation(Pitch, 0, 0);
            const FVector Toe(SpawnXY.X + 120, SpawnXY.Y, 0);
            const FVector RampCenter = Toe + RampRotation.RotateVector(FVector(60, 0, -5));
            const FVector WallCenter(SpawnXY.X + 165, SpawnXY.Y, 150);
            if (!SpawnBox(TEXT("AuditTransitionRamp"), RampCenter, RampRotation, FVector(60, 100, 5), bCanStep) ||
                !SpawnBox(TEXT("AuditRampWitness"), WallCenter, FRotator::ZeroRotator, FVector(5, 100, 150), false))
            {
                return nullptr;
            }
        }
