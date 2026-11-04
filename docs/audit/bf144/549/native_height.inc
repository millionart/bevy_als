        double StepHeightCm;
        if (!(*Case)->TryGetNumberField(TEXT("step_limit_height_cm"), StepHeightCm) ||
            (StepHeightCm != 44.6 && StepHeightCm != 44.9)) { return nullptr; }
        AActor* StepLimit = World->SpawnActor<AActor>();
        if (StepLimit == nullptr) { return nullptr; }
        UBoxComponent* StepLimitBox = NewObject<UBoxComponent>(StepLimit, TEXT("AuditStepLimit"));
        StepLimit->SetRootComponent(StepLimitBox);
        StepLimit->AddInstanceComponent(StepLimitBox);
        StepLimitBox->SetMobility(EComponentMobility::Static);
        StepLimitBox->SetBoxExtent(FVector(25, 100, StepHeightCm * 0.5));
        StepLimitBox->SetWorldLocation(FVector(SpawnXY.X + 160, SpawnXY.Y, StepHeightCm * 0.5));
        StepLimitBox->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
        StepLimitBox->SetCollisionResponseToAllChannels(ECR_Block);
        StepLimitBox->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
        StepLimitBox->RegisterComponent();
