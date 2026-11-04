    struct FModeSample
    {
        EMovementMode Previous;
        EMovementMode Current;
        FVector Location;
        FVector Velocity;
    };

    TArray<FModeSample> Modes;

    UFUNCTION()
    void CaptureMode(ACharacter* Character, EMovementMode Previous, uint8 PreviousCustomMode)
    {
        const UCharacterMovementComponent* Movement = Character->GetCharacterMovement();
        Modes.Add({Previous, Movement->MovementMode.GetValue(), Character->GetActorLocation(), Movement->Velocity});
    }
