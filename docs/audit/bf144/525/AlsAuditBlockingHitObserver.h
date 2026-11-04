#pragma once

#include "CoreMinimal.h"
#include "Engine/HitResult.h"
#include "UObject/Object.h"
#include "AlsAuditBlockingHitObserver.generated.h"

UCLASS()
class UAlsAuditBlockingHitObserver : public UObject
{
    GENERATED_BODY()

public:
    TArray<FHitResult> Hits;

    UFUNCTION()
    void Capture(UPrimitiveComponent* HitComponent, AActor* OtherActor, UPrimitiveComponent* OtherComp,
                 FVector NormalImpulse, const FHitResult& Hit)
    {
        if (NormalImpulse.IsZero() && Hit.bBlockingHit)
        {
            Hits.Add(Hit);
        }
    }
};
