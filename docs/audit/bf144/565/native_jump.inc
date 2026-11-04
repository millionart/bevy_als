            if (Rows.Num() == 49)
            {
                if (bAuditSwitch)
                {
                    AuditNewBase->SetWorldLocation(FVector(SpawnXY.X, SpawnXY.Y, -9.5));
                }
                Box->ComponentVelocity = FVector(100, 0, 0);
                AuditNewBase->ComponentVelocity = FVector(-100, 0, 0);
                Character->Jump();
            }
            else if (Rows.Num() == 50)
            {
                Character->StopJumping();
            }
