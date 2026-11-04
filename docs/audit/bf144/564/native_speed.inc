            Value->SetNumberField(TEXT("max_speed_cm_s"), Movement->GetMaxSpeed());
            Value->SetNumberField(TEXT("max_walk_speed_cm_s"), Movement->MaxWalkSpeed);
            Value->SetNumberField(TEXT("capsule_half_cm"), Character->GetCapsuleComponent()->GetScaledCapsuleHalfHeight());
