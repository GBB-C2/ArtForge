param([switch]$Gpu)
$ErrorActionPreference = "Stop"
Push-Location (Join-Path $PSScriptRoot "..")
try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw "cargo not found; see START_HERE_RU.md" }
    cargo check --workspace --all-targets --locked
    if ($LASTEXITCODE -ne 0) { throw "Workspace compilation failed" }
    cargo test --workspace --locked artistic
    if ($LASTEXITCODE -ne 0) { throw "Artistic material regression tests failed" }
    cargo run -p artforge --release --locked -- --check-shaders
    if ($LASTEXITCODE -ne 0) { throw "Shader validation failed" }
    if ($Gpu) {
        cargo test -p poseforge-render --test gpu --locked artistic -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw "Artistic material GPU tests failed" }
        $Run = Join-Path "output" ("artistic-materials-" + [DateTime]::UtcNow.ToString("yyyyMMdd-HHmmss-ffff"))
        cargo run -p artforge --release --locked -- --render examples/artistic-materials.artforge "$Run/gallery.png" 1440 1080 0
        if ($LASTEXITCODE -ne 0) { throw "Gallery render failed" }
        Write-Host "Gallery output: $Run/gallery.png"
    }
    Write-Host "Requested artistic material checks completed."
} finally { Pop-Location }
