param (
  [string] $shortver = "99.99.99"
)

$ErrorActionPreference = "Stop"
Push-Location "$PSScriptRoot/../../"

. "./ci/build-deps.ps1"

Initialize-Filesystem
Invoke-LinuxBuild
Invoke-LinuxTests
Invoke-DockerBuild

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