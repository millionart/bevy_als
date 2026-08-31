#pragma once

#include "Commandlets/Commandlet.h"

#include "AlsGraphSourceReaderCommandlet.generated.h"

UCLASS()
class ALSGRAPHSOURCEREADER_API UAlsGraphSourceReaderCommandlet : public UCommandlet
{
    GENERATED_BODY()

public:
    UAlsGraphSourceReaderCommandlet();

    virtual int32 Main(const FString& Params) override;
};
