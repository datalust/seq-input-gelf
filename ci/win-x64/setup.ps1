$ErrorActionPreference = "Stop"

Switch-DockerLinux

$RequiredRustToolchain = $(cat ./rust-toolchain)

Invoke-WebRequest -OutFile ./rustup-init.exe -Uri https://win.rustup.rs
$ErrorActionPreference = "Continue"
& ./rustup-init.exe --default-host x86_64-pc-windows-msvc --default-toolchain $RequiredRustToolchain -y
if ($LASTEXITCODE) { exit 1 }

$env:Path = "C:\Users\appveyor\.cargo\bin;$env:Path"

& cargo install -f cross
if ($LASTEXITCODE) { exit 1 }

$ErrorActionPreference = "Stop"
