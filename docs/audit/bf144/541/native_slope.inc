        if ((*Case)->HasField(TEXT("step_slope_yaw")))
        {
            double Yaw, ContactHeight;
            if (!(*Case)->TryGetNumberField(TEXT("step_slope_yaw"), Yaw) ||
                !(*Case)->TryGetNumberField(TEXT("step_slope_contact_height_cm"), ContactHeight) ||
                !FMath::IsFinite(Yaw) || !FMath::IsFinite(ContactHeight)) { return nullptr; }
            AActor* Slope = World->SpawnActor<AActor>();
            if (Slope == nullptr) { return nullptr; }
            UBoxComponent* SlopeBox = NewObject<UBoxComponent>(Slope, TEXT("AuditStepSlope"));
            Slope->SetRootComponent(SlopeBox);
            Slope->AddInstanceComponent(SlopeBox);
            SlopeBox->SetMobility(EComponentMobility::Static);
            SlopeBox->SetBoxExtent(FVector(12, 12, 1.5));
            const FRotator SlopeRotation(0, Yaw, 60);
            const FVector SphereCenter(SpawnXY.X + 133.25, SpawnXY.Y, ContactHeight - 60);
            const FVector Normal = SlopeRotation.RotateVector(FVector::UpVector);
            SlopeBox->SetWorldLocationAndRotation(SphereCenter - 31.5 * Normal, SlopeRotation);
            SlopeBox->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
            SlopeBox->SetCollisionResponseToAllChannels(ECR_Block);
            SlopeBox->GetBodySetup()->CollisionTraceFlag = CTF_UseSimpleAsComplex;
            SlopeBox->RegisterComponent();
        }
