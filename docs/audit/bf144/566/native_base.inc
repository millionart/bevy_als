        bool bAuditStepMoves;
        if (!(*Case)->TryGetBoolField(TEXT("walk_off_step_moves"), bAuditStepMoves)) { return nullptr; }
        AActor* AuditStepActor = World->SpawnActor<AActor>();
        if (AuditStepActor == nullptr) { return nullptr; }
        UBoxComponent* AuditStepBase = NewObject<UBoxComponent>(AuditStepActor, TEXT("AuditWalkOffSecondBase"));
        AuditStepActor->SetRootComponent(AuditStepBase);
        AuditStepActor->AddInstanceComponent(AuditStepBase);
        AuditStepBase->SetMobility(EComponentMobility::Movable);
        AuditStepBase->SetBoxExtent(FVector(7, 100, 10));
        AuditStepBase->SetWorldLocation(FVector(SpawnXY.X + 127, SpawnXY.Y, -12));
        AuditStepBase->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
        AuditStepBase->SetCollisionResponseToAllChannels(ECR_Block);
        AuditStepBase->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
        AuditStepBase->RegisterComponent();
