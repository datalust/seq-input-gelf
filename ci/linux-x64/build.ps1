sudo apt-get update
sudo apt-get install -y libnss3-tools --no-install-recommends

chmod +x ./tool/mkcert-linux-x64
./tool/mkcert-linux-x64 -install

$RequiredRustToolchain=$(cat ./rust-toolchain)

curl https://sh.rustup.rs -sSf | sh -s -- --default-host x86_64-unknown-linux-gnu --default-toolchain $RequiredRustToolchain -y

$env:PATH = "$HOME/.cargo/bin:$env:PATH"

cargo install -f cross

Push-Location "$PSScriptRoot/../../"

. "./ci/build-deps.ps1"

$version = Get-SemVer

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

if ($env:CI_PUBLISH) {
    Publish-Container (Get-SemVer $shortver)
}
else {
    Write-Output "Not publishing Docker container"
}

Pop-Location
