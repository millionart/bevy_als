            if ((*Frame)->HasField(TEXT("falling_initial_cm")))
            {
                const TArray<TSharedPtr<FJsonValue>>* State = nullptr;
                if (!(*Frame)->TryGetArrayField(TEXT("falling_initial_cm"), State) || State->Num() != 6) { return nullptr; }
                double Values[6];
                for (int32 Index = 0; Index < 6; ++Index)
                {
                    if ((*State)[Index]->Type != EJson::Number || !FMath::IsFinite((*State)[Index]->AsNumber())) { return nullptr; }
                    Values[Index] = (*State)[Index]->AsNumber();
                }
                Character->SetActorLocation(FVector(SpawnXY.X + Values[0], SpawnXY.Y + Values[1], Values[2]), false, nullptr, ETeleportType::TeleportPhysics);
                UCharacterMovementComponent* FallingMovement = Character->GetCharacterMovement();
                FallingMovement->SetMovementMode(MOVE_Falling);
                FallingMovement->Velocity = FVector(Values[3], Values[4], Values[5]);
            }
