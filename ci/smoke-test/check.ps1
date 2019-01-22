$ErrorActionPreference = "Stop"
Push-Location "$PSScriptRoot/../../"

. "./ci/build-deps.ps1"

function Start-SeqEnvironment {
    Write-BeginStep $MYINVOCATION

    Push-Location ci/smoke-test

    & docker-compose -p sqelf-test down
    if ($LASTEXITCODE) {
        Pop-Location
        exit 1
    }

    & docker-compose -p sqelf-test rm -f
    if ($LASTEXITCODE) {
        Pop-Location
        exit 1
    }

    if (Test-Path seq-data) {
        Remove-Item -Force -Recurse seq-data
    }

    & docker-compose -p sqelf-test up -d
    if ($LASTEXITCODE) {
        Pop-Location
        exit 1
    }

    # Give Seq enough time to start up
    Start-Sleep -Seconds 5

    Pop-Location
}

function Stop-SeqEnvironment {
    Write-BeginStep $MYINVOCATION

    Push-Location ci/smoke-test

    & docker-compose -p sqelf-test down
    if ($LASTEXITCODE) {
        Pop-Location
        exit 1
    }

    & docker-compose -p sqelf-test rm -f
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

    Invoke-RestMethod -Uri http://localhost:5342/api/events?clef
}

Invoke-NativeBuild
Build-Container
Build-TestAppContainer
Start-SeqEnvironment
Invoke-TestApp
Check-ClefOutput
Stop-SeqEnvironment

Pop-Location