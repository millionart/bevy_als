        double FrontCm;
        if (!(*Case)->TryGetNumberField(TEXT("step_stop_front_cm"), FrontCm) ||
            !FMath::IsFinite(FrontCm) || FrontCm < 138 || FrontCm > 139) { return nullptr; }
        AActor* StopWall = World->SpawnActor<AActor>();
        if (StopWall == nullptr) { return nullptr; }
        UBoxComponent* StopBox = NewObject<UBoxComponent>(StopWall, TEXT("AuditStepStop"));
        StopWall->SetRootComponent(StopBox);
        StopWall->AddInstanceComponent(StopBox);
        StopBox->SetMobility(EComponentMobility::Static);
        StopBox->SetBoxExtent(FVector(4, 50, 35));
        StopBox->SetWorldLocation(FVector(SpawnXY.X + FrontCm + 4, SpawnXY.Y, 215));
        StopBox->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
        StopBox->SetCollisionResponseToAllChannels(ECR_Block);
        StopBox->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
        StopBox->RegisterComponent();
