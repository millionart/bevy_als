#!/bin/bash
set -eu
exec pwsh -NoProfile -File "$(dirname "$0")/run_cmdlet.ps1" -Script "$@"
