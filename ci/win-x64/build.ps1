param (
  [string] $shortver = "99.99.99"
)

$ErrorActionPreference = "Stop"
Push-Location "$PSScriptRoot/../../"

. "./ci/build-deps.ps1"

Initialize-Filesystem
Invoke-WindowsBuild
Invoke-WindowsTests
Invoke-LinuxBuild
Invoke-NuGetPack $shortver

Pop-Location