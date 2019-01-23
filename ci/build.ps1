param (
  [string] $shortver = "99.99.99"
)

$ErrorActionPreference = "Stop"
Push-Location "$PSScriptRoot/../"

. "./ci/build-deps.ps1"

Initialize-Docker
Initialize-Filesystem
Invoke-LinuxBuild
Invoke-DockerBuild

if ($IsWindows) {
    Invoke-WindowsBuild
    Invoke-NuGetPack $shortver
}
else {
    Write-Output "Not running Windows build"
}

Build-TestAppContainer
Start-SeqEnvironment
Invoke-TestApp
Check-SqelfLogs
Check-SeqLogs
Check-ClefOutput
Stop-SeqEnvironment

if ($IsPublishedBuild) {
    Publish-Container $shortver
}
else {
    Write-Output "Not publishing Docker container"
}

Pop-Location