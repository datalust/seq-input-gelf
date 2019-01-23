$ErrorActionPreference = "Stop"
Push-Location "$PSScriptRoot/../../"

. "./ci/build-deps.ps1"

function Start-SeqEnvironment {
    Write-BeginStep $MYINVOCATION

    Push-Location ci/smoke-test

    & docker rm -f sqelf-test-seq | Out-Null
    & docker rm -f sqelf-test-sqelf | Out-Null

    & docker network rm sqelf-test | Out-Null

    if (Test-Path seq-data) {
        Remove-Item -Force -Recurse seq-data
    }

    $data = "$(pwd)/seq-data"

    & docker network create sqelf-test
    if ($LASTEXITCODE) {
        Pop-Location
        exit 1
    }

    & docker run --name sqelf-test-seq `
        --network sqelf-test `
        -e ACCEPT_EULA=Y `
        -itd `
        -v "${data}:/data" `
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

    Remove-Item -Force -Recurse seq-data

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

# Build-TestAppContainer
Start-SeqEnvironment
Invoke-TestApp
Check-SqelfLogs
Check-SeqLogs
Check-ClefOutput
Stop-SeqEnvironment

Pop-Location