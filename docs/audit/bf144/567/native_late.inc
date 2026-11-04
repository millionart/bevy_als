            TSharedPtr<FJsonObject> LateBase;
            if (Rows.Num() == 40)
            {
                HitObserver->AuditAfterMovement = [&]
                {
                    LateBase = MakeShared<FJsonObject>();
                    LateBase->SetStringField(TEXT("actor_base"), GetNameSafe(Character->GetMovementBase()));
                    LateBase->SetObjectField(TEXT("actor_cm"), CaptureVector(Character->GetActorLocation()));
                    LateBase->SetObjectField(TEXT("old_before_cm"), CaptureVector(Box->GetComponentLocation()));
                    if (bMoveOldBaseLate) { Box->AddWorldOffset(FVector(0, 20, 0), false, nullptr, ETeleportType::TeleportPhysics); }
                    LateBase->SetObjectField(TEXT("old_after_cm"), CaptureVector(Box->GetComponentLocation()));
                };
                Character->OnCharacterMovementUpdated.AddDynamic(HitObserver.Get(), &UAlsAuditBlockingHitObserver::CaptureAfterMovement);
            }
            World->Tick(LEVELTICK_All, static_cast<float>(Delta));
            if (Rows.Num() == 40)
            {
                Character->OnCharacterMovementUpdated.RemoveDynamic(HitObserver.Get(), &UAlsAuditBlockingHitObserver::CaptureAfterMovement);
                HitObserver->AuditAfterMovement = {};
            }
