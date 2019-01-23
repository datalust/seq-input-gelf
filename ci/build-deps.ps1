$IsCIBuild = $null -ne $env:APPVEYOR_BUILD_NUMBER
$IsPublishedBuild = $env:APPVEYOR_REPO_BRANCH -eq "master" -and $null -eq $env:APPVEYOR_PULL_REQUEST_HEAD_REPO_BRANCH

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
        Push-Location "$hostShare/src"
    }

    & "./ci/cross-build.ps1" 2>&1
    if ($LASTEXITCODE) { exit 1 }
    
    if ($IsCIBuild) {
        Pop-Location
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

    # Cargo writes to STDERR
    $ErrorActionPreference = "SilentlyContinue"

    cargo build --release --target=x86_64-pc-windows-msvc
    if ($LASTEXITCODE) { exit 1 }

    $ErrorActionPreference = "Stop"
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

function Start-SeqEnvironment {
    Write-BeginStep $MYINVOCATION

    if ($IsCIBuild) {
        $hostShare = "X:\host"
        Push-Location "$hostShare/src"
    }

    Push-Location ci/smoke-test

    $ErrorActionPreference = "SilentlyContinue"

    & docker rm -f sqelf-test-seq | Out-Null
    & docker rm -f sqelf-test-sqelf | Out-Null

    & docker network rm sqelf-test | Out-Null

    & docker network create sqelf-test
    if ($LASTEXITCODE) {
        Pop-Location
        exit 1
    }

    & docker run --name sqelf-test-seq `
        --network sqelf-test `
        -e ACCEPT_EULA=Y `
        -itd `
        -p 5342:80 `
        datalust/seq:latest
    if ($LASTEXITCODE) {
        Pop-Location
        exit 1
    }

    & docker run --name sqelf-test-sqelf `
        --network sqelf-test `
        -e SEQ_ADDRESS=http://sqelf-test-seq:5341 `
        -itd `
        -p 12202:12201/udp `
        sqelf-ci:latest
    if ($LASTEXITCODE) {
        Pop-Location
        exit 1
    }

    # Give Seq enough time to start up
    Start-Sleep -Seconds 5

    if ($IsCIBuild) {
        Pop-Location
    }

    $ErrorActionPreference = "Stop"

    Pop-Location
}

function Stop-SeqEnvironment {
    Write-BeginStep $MYINVOCATION

    Push-Location ci/smoke-test

    & docker rm -f sqelf-test-seq
    if ($LASTEXITCODE) {
        Pop-Location
        exit 1
    }

    & docker rm -f sqelf-test-sqelf
    if ($LASTEXITCODE) {
        Pop-Location
        exit 1
    }

    & docker network rm sqelf-test
    if ($LASTEXITCODE) {
        Pop-Location
        exit 1
    }

    Pop-Location
}

function Build-TestAppContainer {
    Write-BeginStep $MYINVOCATION

    & docker build --file ci/smoke-test/app/Dockerfile -t sqelf-app-test:latest .
    if ($LASTEXITCODE) { exit 1 }
}

function Invoke-TestApp {
    Write-BeginStep $MYINVOCATION

    & docker run `
        --rm `
        -it `
        --log-driver gelf `
        --log-opt gelf-address=udp://localhost:12202 `
        sqelf-app-test:latest
    if ($LASTEXITCODE) { exit 1 }

    # Give sqelf enough time to batch and send
    Start-Sleep -Seconds 2
}

function Check-ClefOutput {
    Write-BeginStep $MYINVOCATION

    $json = Invoke-RestMethod -Uri http://localhost:5342/api/events?clef

    if (-Not $json) {
        throw [System.Exception] "CLEF output is empty"
    } else {
        $json
    }
}

function Check-SqelfLogs {
    Write-BeginStep $MYINVOCATION

    & docker logs sqelf-test-sqelf
}

function Check-SeqLogs {
    Write-BeginStep $MYINVOCATION

    & docker logs sqelf-test-seq
}