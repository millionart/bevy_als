        AActor* LowerFloor = World->SpawnActor<AActor>();
        if (LowerFloor == nullptr) { return nullptr; }
        UBoxComponent* LowerFloorBox = NewObject<UBoxComponent>(LowerFloor, TEXT("AuditLowerFloor"));
        LowerFloor->SetRootComponent(LowerFloorBox);
        LowerFloor->AddInstanceComponent(LowerFloorBox);
        LowerFloorBox->SetMobility(EComponentMobility::Static);
        LowerFloorBox->SetBoxExtent(FVector(2000, 1500, 10));
        LowerFloorBox->SetWorldLocation(FVector(SpawnXY.X, SpawnXY.Y, -210));
        LowerFloorBox->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
        LowerFloorBox->SetCollisionResponseToAllChannels(ECR_Block);
        LowerFloorBox->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
        LowerFloorBox->RegisterComponent();
