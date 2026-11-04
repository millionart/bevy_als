        double StuckGap = 0;
        if (!(*Case)->TryGetNumberField(TEXT("stuck_gap_cm"), StuckGap)) { return nullptr; }
        if (StuckGap > 0)
        {
            AActor* Wall = World->SpawnActor<AActor>();
            if (Wall == nullptr) { return nullptr; }
            UBoxComponent* Panel = NewObject<UBoxComponent>(Wall, TEXT("AuditStuckRight"));
            Wall->SetRootComponent(Panel);
            Wall->AddInstanceComponent(Panel);
            Panel->SetMobility(EComponentMobility::Static);
            Panel->SetBoxExtent(FVector(25, 100, 100));
            Panel->SetWorldLocation(FVector(SpawnXY.X + 152 + StuckGap * 0.5 + 25, SpawnXY.Y, 170));
            Panel->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
            Panel->SetCollisionResponseToAllChannels(ECR_Block);
            Panel->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
            Panel->RegisterComponent();
        }
