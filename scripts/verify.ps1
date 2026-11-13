param([switch]$Gpu)
$ErrorActionPreference = "Stop"
Push-Location (Join-Path $PSScriptRoot "..")
try {
    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw "cargo fmt failed" }
    cargo clippy --workspace --all-targets --locked -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw "cargo clippy failed" }
    cargo test --workspace --locked
    if ($LASTEXITCODE -ne 0) { throw "cargo test failed" }
    cargo run --locked -- --self-check output/self-check
    if ($LASTEXITCODE -ne 0) { throw "self-check failed" }
    if ($Gpu) {
        $Run = Join-Path "output" ("gpu-check-" + [DateTime]::UtcNow.ToString("yyyyMMdd-HHmmss-ffff"))
        cargo run --locked -- --render demo "$Run/single.png" 640 360 0.5
        if ($LASTEXITCODE -ne 0) { throw "PNG render failed" }
        cargo run --locked -- --sequence demo "$Run/sequence" 12 12 640 360
        if ($LASTEXITCODE -ne 0) { throw "Sequence render failed" }
        $Single = (Get-FileHash "$Run/single.png" -Algorithm SHA256).Hash
        $Sequence = (Get-FileHash "$Run/sequence/frame_000012.png" -Algorithm SHA256).Hash
        if ($Single -ne $Sequence) { throw "Single frame differs from sequence at the same time" }
        cargo run --locked -- --render demo "$Run/transparent.png" 640 360 0.5 --transparent
        if ($LASTEXITCODE -ne 0) { throw "Transparent PNG render failed" }
        cargo run --locked -- --sequence demo "$Run/sequence" 12 12 640 360 --resume
        if ($LASTEXITCODE -ne 0) { throw "Sequence resume failed" }
        Write-Host "GPU outputs: $Run"
    }
    Write-Host "All requested checks completed."
} finally { Pop-Location }
