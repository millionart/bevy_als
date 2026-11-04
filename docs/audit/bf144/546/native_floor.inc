        double FloorPitch;
        if (!(*Case)->TryGetNumberField(TEXT("step_floor_pitch"), FloorPitch) ||
            (FloorPitch != 4 && FloorPitch != -4)) { return nullptr; }
        const FRotator AuditFloorRotation(FloorPitch, -25, 0);
        const FVector AuditFloorTop(SpawnXY.X + 102, SpawnXY.Y, 0);
        Box->SetBoxExtent(FVector(FloorHalfExtentX, 150, 10));
        Box->SetWorldLocationAndRotation(AuditFloorTop - AuditFloorRotation.RotateVector(FVector::UpVector) * 10, AuditFloorRotation);
