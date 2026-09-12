# Forwarding alias for parallel.ps1
& (Join-Path $PSScriptRoot 'parallel.ps1') @args
exit $LASTEXITCODE
