param (
  [string] $shortver = "99.99.99"
)

$ErrorActionPreference = "Stop"
Push-Location "$PSScriptRoot/../"

. "./ci/build-deps.ps1"

$version = "$shortver.0"

Initialize-Docker
Initialize-HostShare
Invoke-NativeBuild
Build-Container

if ($IsPublishedBuild) {
    Publish-Container $version
}
else {
    Write-Output "Not publishing Docker container"
}

Pop-Location