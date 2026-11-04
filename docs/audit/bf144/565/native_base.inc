        bool bAuditSwitch;
        if (!(*Case)->TryGetBoolField(TEXT("jump_base_switch"), bAuditSwitch)) { return nullptr; }
        AActor* AuditNewActor = World->SpawnActor<AActor>();
        if (AuditNewActor == nullptr) { return nullptr; }
        UBoxComponent* AuditNewBase = NewObject<UBoxComponent>(AuditNewActor, TEXT("AuditJumpBaseNew"));
        AuditNewActor->SetRootComponent(AuditNewBase);
        AuditNewActor->AddInstanceComponent(AuditNewBase);
        AuditNewBase->SetMobility(EComponentMobility::Movable);
        AuditNewBase->SetBoxExtent(FVector(100, 100, 10));
        AuditNewBase->SetWorldLocation(FVector(SpawnXY.X, SpawnXY.Y, -15));
        AuditNewBase->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
        AuditNewBase->SetCollisionResponseToAllChannels(ECR_Block);
        AuditNewBase->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
        AuditNewBase->RegisterComponent();
