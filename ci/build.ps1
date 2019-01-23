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

if ($IsPublishedBuild) {
    Publish-Container $shortver
}
else {
    Write-Output "Not publishing Docker container"
}

Pop-Location