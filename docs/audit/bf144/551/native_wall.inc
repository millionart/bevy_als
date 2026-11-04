        double AuditWallFront;
        if (!(*Case)->TryGetNumberField(TEXT("stall_front_cm"), AuditWallFront) ||
            (AuditWallFront != 132.25 && AuditWallFront != 136)) { return nullptr; }
        AActor* StopWall = World->SpawnActor<AActor>();
        if (StopWall == nullptr) { return nullptr; }
        UBoxComponent* StopBox = NewObject<UBoxComponent>(StopWall, TEXT("AuditStallWall"));
        StopWall->SetRootComponent(StopBox);
        StopWall->AddInstanceComponent(StopBox);
        StopBox->SetMobility(EComponentMobility::Static);
        StopBox->SetBoxExtent(FVector(5, 100, 150));
        StopBox->SetWorldLocation(FVector(SpawnXY.X + AuditWallFront + 5, SpawnXY.Y, 150));
        StopBox->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
        StopBox->SetCollisionResponseToAllChannels(ECR_Block);
        StopBox->CanCharacterStepUpOn = ECB_No;
        StopBox->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
        StopBox->RegisterComponent();
