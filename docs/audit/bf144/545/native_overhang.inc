        if ((*Case)->HasField(TEXT("step_overhang")))
        {
            bool bOverhang;
            if (!(*Case)->TryGetBoolField(TEXT("step_overhang"), bOverhang) || !bOverhang) { return nullptr; }
            AActor* Actor = World->SpawnActor<AActor>();
            if (Actor == nullptr) { return nullptr; }
            UBoxComponent* OverhangBox = NewObject<UBoxComponent>(Actor, TEXT("AuditStepOverhang"));
            Actor->SetRootComponent(OverhangBox);
            Actor->AddInstanceComponent(OverhangBox);
            OverhangBox->SetMobility(EComponentMobility::Static);
            OverhangBox->SetBoxExtent(FVector(2, 50, 75));
            const FRotator Rotation(30, -45, 0);
            const FVector Center = FVector(SpawnXY.X + 113, SpawnXY.Y, 195) + Rotation.Vector() * 32;
            OverhangBox->SetWorldLocationAndRotation(Center, Rotation);
            OverhangBox->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
            OverhangBox->SetCollisionResponseToAllChannels(ECR_Block);
            OverhangBox->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
            OverhangBox->RegisterComponent();
        }
