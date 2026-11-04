        bool bStepFin = false;
        if ((*Case)->HasField(TEXT("step_fin")) && !(*Case)->TryGetBoolField(TEXT("step_fin"), bStepFin)) { return nullptr; }
        if (bStepFin)
        {
            AActor* Fin = World->SpawnActor<AActor>();
            if (Fin == nullptr) { return nullptr; }
            UBoxComponent* FinBox = NewObject<UBoxComponent>(Fin, TEXT("AuditStepFin"));
            Fin->SetRootComponent(FinBox);
            Fin->AddInstanceComponent(FinBox);
            FinBox->SetMobility(EComponentMobility::Static);
            FinBox->SetBoxExtent(FVector(1, 1, 10));
            FinBox->SetWorldLocation(FVector(SpawnXY.X + 114.5, SpawnXY.Y + 27, 8));
            FinBox->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
            FinBox->SetCollisionResponseToAllChannels(ECR_Block);
            FinBox->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
            FinBox->RegisterComponent();
        }
