$IsCIBuild = $null -ne $env:APPVEYOR_BUILD_NUMBER
$IsPublishedBuild = $IsCIBuild -and $null -eq $env:APPVEYOR_PULL_REQUEST_HEAD_REPO_BRANCH

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

function Get-SemVer($shortver)
{
    # This script originally (c) 2016 Serilog Contributors - license Apache 2.0
    $branch = @{ $true = $env:APPVEYOR_REPO_BRANCH; $false = $(git symbolic-ref --short -q HEAD) }[$env:APPVEYOR_REPO_BRANCH -ne $NULL];
    $suffix = @{ $true = ""; $false = ($branch.Substring(0, [math]::Min(10,$branch.Length)) -replace '[\/\+]','-').Trim("-")}[$branch -eq "master"]

    if ($suffix) {
        $shortver + "-" + $suffix
    } else {
        $shortver
    }
}

function Run-Command
{
    Param ($Exe, $ArgumentList)

    # For commands that treat stderr like stdetc
    $out = New-TemporaryFile
    $err = New-TemporaryFile
    $r = Start-Process $Exe -ArgumentList $ArgumentList -Wait -PassThru -RedirectStandardOut $out.FullName -RedirectStandardError $err.FullName

    Write-Output "STDOUT"
    Get-Content -Path $out.FullName
    Write-Output ""

    Write-Output "STDERR"
    Get-Content -Path $err.FullName
    Write-Output ""
    
    Remove-Item $out.FullName
    Remove-Item $err.FullName

    if ($r.ExitCode -ne 0) {
        exit $r.ExitCode
    }
}

function Initialize-Filesystem
{
    Write-BeginStep $MYINVOCATION

    if (Test-Path .\publish)
    {
        Remove-Item -Recurse -Force .\publish
    }

    New-Item -ItemType Directory .\publish
}

function Invoke-LinuxBuild
{
    Write-BeginStep $MYINVOCATION

    Run-Command -Exe cargo -ArgumentList 'build', '--bin sqelf', '--release', '--target x86_64-unknown-linux-musl'
}
function Invoke-LinuxTests
{
    Write-BeginStep $MYINVOCATION

    Run-Command -Exe cargo -ArgumentList 'test', '--target x86_64-unknown-linux-musl'
    Run-Command -Exe cargo -ArgumentList 'run', '-p sqelf_tests', '--target x86_64-unknown-linux-musl'
}

function Invoke-DockerBuild
{
    Write-BeginStep $MYINVOCATION

    docker build --no-cache --file dockerfiles/Dockerfile -t sqelf-ci:latest .
    if ($LASTEXITCODE) { exit 1 }
}

function Invoke-WindowsBuild
{
    Write-BeginStep $MYINVOCATION

    Run-Command -Exe cargo -ArgumentList 'build', '--bin sqelf', '--release', '--target x86_64-pc-windows-msvc'
}
function Invoke-WindowsTests
{
    Write-BeginStep $MYINVOCATION

    Run-Command -Exe cargo -ArgumentList 'test', '--target x86_64-pc-windows-msvc'
    Run-Command -Exe cargo -ArgumentList 'run', '-p sqelf_tests', '--target x86_64-pc-windows-msvc'
}

function Invoke-NuGetPack($version)
{
    Write-BeginStep $MYINVOCATION

    .\tool\nuget.exe pack .\Seq.Input.Gelf.nuspec -version $version -outputdirectory .\publish
    if ($LASTEXITCODE) { exit 1 }
}

function Publish-Container($version)
{
    Write-BeginStep $MYINVOCATION

    docker tag sqelf-ci:latest datalust/sqelf-ci:$version
    if ($LASTEXITCODE) { exit 1 }

    if ($IsCIBuild)
    {
        echo "$env:DOCKER_TOKEN" | docker login -u $env:DOCKER_USER --password-stdin
        if ($LASTEXITCODE) { exit 1 }
    }

    docker push datalust/sqelf-ci:$version
    if ($LASTEXITCODE) { exit 1 }
}

function Start-SeqEnvironment($protocol) {
    Write-BeginStep $MYINVOCATION

    Push-Location ci/smoke-test

    $ErrorActionPreference = "SilentlyContinue"

    docker rm -f sqelf-test-seq | Out-Null
    docker rm -f sqelf-test-sqelf | Out-Null

    docker network rm sqelf-test | Out-Null

    docker network create sqelf-test
    if ($LASTEXITCODE) {
        Pop-Location
        exit 1
    }

    $portArg = "12201"
    if ($protocol -eq "udp") {
        $portArg = "12201/udp"
    }

    docker pull datalust/seq:latest
    docker run --name sqelf-test-seq `
        --network sqelf-test `
        -e ACCEPT_EULA=Y `
        -itd `
        -p 5342:80 `
        datalust/seq:latest
    if ($LASTEXITCODE) {
        Pop-Location
        exit 1
    }

    docker run --name sqelf-test-sqelf `
        --network sqelf-test `
        -e SEQ_ADDRESS=http://sqelf-test-seq:5341 `
        -e GELF_ADDRESS="${protocol}://0.0.0.0:12201" `
        -itd `
        -p "12202:${portArg}" `
        sqelf-ci:latest
    if ($LASTEXITCODE) {
        Pop-Location
        exit 1
    }

    # Give Seq enough time to start up
    Start-Sleep -Seconds 5

    $ErrorActionPreference = "Stop"

    Pop-Location
}

function Stop-SeqEnvironment {
    Write-BeginStep $MYINVOCATION

    Push-Location ci/smoke-test

    docker rm -f sqelf-test-seq
    if ($LASTEXITCODE) {
        Pop-Location
        exit 1
    }

    docker rm -f sqelf-test-sqelf
    if ($LASTEXITCODE) {
        Pop-Location
        exit 1
    }

    docker network rm sqelf-test
    if ($LASTEXITCODE) {
        Pop-Location
        exit 1
    }

    Pop-Location
}

function Build-TestAppContainer {
    Write-BeginStep $MYINVOCATION

    docker build --file ci/smoke-test/app/Dockerfile -t sqelf-app-test:latest .
    if ($LASTEXITCODE) { exit 1 }
}

function Invoke-TestApp($protocol) {
    Write-BeginStep $MYINVOCATION

    docker run `
        --rm `
        -i `
        --log-driver gelf `
        --log-opt "gelf-address=${protocol}://localhost:12202" `
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

    docker logs sqelf-test-sqelf
}

function Check-SeqLogs {
    Write-BeginStep $MYINVOCATION

    docker logs sqelf-test-seq
}
