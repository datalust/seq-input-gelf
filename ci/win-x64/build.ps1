param (
  [string] $shortver = "99.99.99",
  [string] $branch = "dev"
)

$ErrorActionPreference = "Stop"
Push-Location "$PSScriptRoot/../../"

if ($branch -ne "main") {
    $shortver = "$shortver-$branch"
}

. "./ci/build-deps.ps1"

Initialize-Filesystem
Invoke-WindowsBuild
Invoke-WindowsTests
Invoke-LinuxBuild
Invoke-NuGetPack (Get-SemVer $shortver)

Pop-Location
