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

function Initialize-Filesystem
{
    Write-BeginStep $MYINVOCATION
    
    if (Test-Path .\publish) {
        Remove-Item .\publish -Recurse
    }

    mkdir .\publish

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

function Invoke-LinuxBuild
{
    Write-BeginStep $MYINVOCATION

    if ($IsCIBuild) {
        $hostShare = "X:\host"
        pushd "$hostShare/src"
    }

    & "./ci/cross-build.ps1" 2>&1
    if ($LASTEXITCODE) { exit 1 }
    
    if ($IsCIBuild) {
        popd
        Copy-Item -Path "$hostShare/src/target" -Recurse -Destination . -Container
    }
}

function Invoke-DockerBuild
{
    Write-BeginStep $MYINVOCATION

    & docker build --file dockerfiles/Dockerfile -t sqelf-ci:latest .
    if ($LASTEXITCODE) { exit 1 }
}

function Invoke-WindowsBuild
{
    Write-BeginStep $MYINVOCATION

    cargo build --release --target=x86_64-pc-windows-msvc --verbose
    if ($LASTEXITCODE) { exit 1 }
}

function Invoke-NuGetPack($version)
{
    Write-BeginStep $MYINVOCATION

    & .\tool\nuget.exe pack .\Seq.Input.Gelf.nuspec -version $version -outputdirectory .\publish
    if ($LASTEXITCODE) { exit 1 }
}

function Publish-Container($version)
{
    Write-BeginStep $MYINVOCATION

    & docker tag sqelf-ci:latest datalust/sqelf-ci:$version
    if ($LASTEXITCODE) { exit 1 }

    if ($IsCIBuild)
    {
        echo "$env:DOCKER_TOKEN" | docker login -u $env:DOCKER_USER --password-stdin
        if ($LASTEXITCODE) { exit 1 }
    }

    & docker push datalust/sqelf-ci:$version
    if ($LASTEXITCODE) { exit 1 }
}

$ErrorActionPreference = "Stop"
Push-Location "$PSScriptRoot/../"

Initialize-Docker
Initialize-Filesystem
Invoke-LinuxBuild
Invoke-DockerBuild
Invoke-WindowsBuild
Invoke-NuGetPack $shortver

if ($IsPublishedBuild) {
    Publish-Container $shortver
}
else {
    Write-Output "Not publishing Docker container"
}
