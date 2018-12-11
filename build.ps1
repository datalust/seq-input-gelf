param (
  [string] $shortver = "99.99.99"
)

$IsCIBuild = $null -ne $env:APPVEYOR_BUILD_NUMBER
$IsPublishedBuild = $env:APPVEYOR_REPO_BRANCH -eq "master" -and $null -eq $env:APPVEYOR_PULL_REQUEST_HEAD_REPO_BRANCH

function Write-BeginStep($invocation)
{
    Write-Output ""
    Write-Output "###########################################################"
    Write-Output "# $($invocation.MyCommand)"
    foreach ($key in  $invocation.BoundParameters.Keys) {
        Write-Output "#   $($key): $($invocation.BoundParameters[$key])"
    }
    Write-Output "###########################################################"
    Write-Output ""
}

function Initialize-Docker
{
    Write-BeginStep $MYINVOCATION
    
    if ($IsCIBuild) {
        Write-Output "Switching Docker to Linux containers..."
        
        docker-switch-linux
        if ($LASTEXITCODE) { exit 1 }
    }
}

function Initialize-HostShare
{
    Write-BeginStep $MYINVOCATION
    
    if ($IsCIBuild)
    {
        $hostShare = "X:\host"
        ls $hostshare

        mkdir "$hostShare/src"
        Copy-Item -Path ./* -Recurse -Destination "$hostShare/src"

        mkdir "$hostShare\tmp"
        $env:TMP = "$hostShare\tmp"
        $env:TEMP = "$hostShare\tmp"
    }
}

function Invoke-NativeBuild
{
    Write-BeginStep $MYINVOCATION

    if ($IsCIBuild) {
        $hostShare = "X:\host"
        pushd "$hostShare/src"
    }

    & "./ci/native/cross-build.ps1" 2>&1
    if ($LASTEXITCODE) { exit 1 }
    
    if ($IsCIBuild) {
        popd
        Copy-Item -Path "$hostShare/src/target" -Recurse -Destination . -Container
    }
}

function Build-Container($semver)
{
    Write-BeginStep $MYINVOCATION

    & docker build --file dockerfiles/Dockerfile -t datalust/sqelf-ci:$semver .
    if ($LASTEXITCODE) { exit 1 }
}

$ErrorActionPreference = "Stop"
Push-Location $PSScriptRoot

$suffix = $null
if (!$IsCIBuild) {
    $suffix = "-local"
}

$semver = $shortver
if ($suffix) {
    $semver = "$shortver$suffix"
}

$version = "$shortver.0"

Initialize-Docker
Initialize-HostShare
Invoke-NativeBuild
Build-Container($semver)
