param (
  [string] $shortver = "99.99.99"
)

$ErrorActionPreference = "Stop"
Push-Location "$PSScriptRoot/../../"

. "./ci/build-deps.ps1"

function Invoke-SmokeTest($protocol) {
    Write-BeginStep $MYINVOCATION

    $finished = $false
    $retries = 0

    do {
        try {
            Start-SeqEnvironment($protocol)
            Invoke-TestApp($protocol)
            Check-SqelfLogs
            Check-SeqLogs
            Check-ClefOutput

            $finished = $true
        }
        catch {
            Stop-SeqEnvironment

            if ($retries -gt 3) {
                exit 1
            }
            else {
                $retries = $retries + 1
                Write-Host "Retrying (attempt $retries)"
            }
        }
        
    }
    while ($finished -eq $false)

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
