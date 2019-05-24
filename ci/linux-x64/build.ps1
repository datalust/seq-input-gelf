param (
  [string] $shortver = "99.99.99"
)

$ErrorActionPreference = "Stop"
Push-Location "$PSScriptRoot/../../"

. "./ci/build-deps.ps1"

function Invoke-SmokeTest($protocol) {
    Write-BeginStep $MYINVOCATION

    Start-SeqEnvironment($protocol)
    Invoke-TestApp($protocol)
    Check-SqelfLogs
    Check-SeqLogs
    Check-ClefOutput
    Stop-SeqEnvironment
}

Initialize-Filesystem
Invoke-LinuxBuild
Invoke-LinuxTests
Invoke-DockerBuild

Build-TestAppContainer

Invoke-SmokeTest("udp")
Invoke-SmokeTest("tcp")

if ($IsPublishedBuild) {
    Publish-Container (Get-SemVer $shortver)
}
else {
    Write-Output "Not publishing Docker container"
}

Pop-Location
