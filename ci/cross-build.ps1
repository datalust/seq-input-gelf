$pwd=(pwd).tostring()

$toolchain = 'stable'
if (Test-Path env:RUST_TOOLCHAIN) {
    $toolchain = $env:RUST_TOOLCHAIN
}

Push-Location ci
& docker build --build-arg TOOLCHAIN=$toolchain -t sqelf-build:latest .
if ($LASTEXITCODE) { exit 1 }
Pop-Location

& docker run -it `
    -e SQELF_TEST=$SQELF_TEST `
    -e SQELF_NATIVE_TEST=$SQELF_NATIVE_TEST `
    -v ${pwd}:/sqelf `
    sqelf-build:latest /bin/bash `
    -c "cd /sqelf;./ci/local-build.sh"
if ($LASTEXITCODE) { exit 1 }
