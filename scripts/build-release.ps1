# Build a fresh Windows x64 portable package from the current source tree.
$ErrorActionPreference = "Stop"
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT -or -not [Environment]::Is64BitOperatingSystem) {
    throw "Run this script on Windows x64 with Rust and C++ Build Tools installed."
}
$null = Get-Command cargo -ErrorAction Stop
$null = Get-Command rustc -ErrorAction Stop
$ProjectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
Push-Location $ProjectRoot
try {
    $Manifest = Get-Content -LiteralPath "Cargo.toml" -Raw
    if ($Manifest -notmatch '(?ms)^\[workspace\.package\]\s*\r?\n(?<body>.*?)(?=^\[|\z)') {
        throw "Cannot find workspace.package in Cargo.toml."
    }
    $WorkspacePackage = $Matches['body']
    if ($WorkspacePackage -notmatch '(?m)^version\s*=\s*"(?<version>[0-9]+\.[0-9]+\.[0-9]+(?:-[A-Za-z0-9.-]+)?)"\s*$') {
        throw "Cannot determine the package version from Cargo.toml."
    }
    $Version = $Matches['version']
    $Target = "x86_64-pc-windows-msvc"
    cargo build --release --locked --target $Target
    if ($LASTEXITCODE -ne 0) { throw "Release build failed; no new package was created." }

    # Use Cargo metadata so a configured CARGO_TARGET_DIR is honored.
    $MetadataJson = cargo metadata --no-deps --format-version 1 --locked
    if ($LASTEXITCODE -ne 0) { throw "Cannot locate the Cargo target directory." }
    $Metadata = $MetadataJson | ConvertFrom-Json
    $Binary = Join-Path $Metadata.target_directory "$Target/release/artforge.exe"
    if (-not (Test-Path -LiteralPath $Binary -PathType Leaf)) {
        throw "The completed build did not produce $Binary."
    }
    $RustVersion = & rustc --version
    if ($LASTEXITCODE -ne 0) { throw "Cannot record rustc version." }
    $CargoVersion = & cargo --version
    if ($LASTEXITCODE -ne 0) { throw "Cannot record Cargo version." }

    $Dist = Join-Path $ProjectRoot "dist"
    $PackageName = "ArtForge-Studio-$Version-Windows-x64"
    $Package = Join-Path $Dist $PackageName
    $Archive = Join-Path $Dist "$PackageName.zip"
    $PendingArchive = Join-Path $Dist "$PackageName.partial.zip"
    New-Item -ItemType Directory -Path $Dist -Force | Out-Null
    # This exact directory is script-owned staging. Never merge an old package.
    if (Test-Path -LiteralPath $Package) { Remove-Item -LiteralPath $Package -Recurse -Force }
    if (Test-Path -LiteralPath $PendingArchive) { Remove-Item -LiteralPath $PendingArchive -Force }
    New-Item -ItemType Directory -Path $Package | Out-Null
    Copy-Item -LiteralPath $Binary -Destination (Join-Path $Package "artforge.exe")
    foreach ($File in @("README.md", "CHANGELOG.md", "LICENSE", "THIRD_PARTY_NOTICES.md", "Cargo.lock", "rust-toolchain.toml")) {
        Copy-Item -LiteralPath (Join-Path $ProjectRoot $File) -Destination (Join-Path $Package $File)
    }
    foreach ($Directory in @("docs", "examples", "third-party-licenses")) {
        Copy-Item -LiteralPath (Join-Path $ProjectRoot $Directory) -Destination (Join-Path $Package $Directory) -Recurse
    }
    Copy-Item -LiteralPath (Join-Path $ProjectRoot "START_HERE_RU.md") -Destination (Join-Path $Package "SOURCE_NOTES_RU.md")
    @"
# ArtForge Studio $Version — Windows x64

Это переносимая сборка приложения. Для её запуска Rust не нужен.

1. Полностью распакуйте архив в папку, доступную для записи.
2. Запустите START_WINDOWS.cmd или artforge.exe.
3. Используйте демонстрационную сцену либо импортируйте GLB/glTF или .blend.
   Для .blend отдельно нужен Blender 3.6+ с совместимым glTF-экспортёром.
   Укажите путь к Blender в окне «Импорт», если он не найден автоматически.

Нужен совместимый графический драйвер. Если запуск завершается ошибкой,
откройте START_WINDOWS.cmd, чтобы прочитать сообщение в консоли.

BUILD.txt содержит время, целевую платформу и версии инструментов этой
сборки. SHA256SUMS.txt содержит контрольную сумму исполняемого файла.
Успешная сборка сама по себе не подтверждает запуск на вашем компьютере,
работу графического драйвера, проверку интерфейса или прохождение тестов.

README.md, SOURCE_NOTES_RU.md и docs/VALIDATION.md сохраняют описание
передачи исходников до этой сборки. Их указания об отсутствии нового EXE
и необходимости Cargo относятся к исходному архиву; этот переносимый
архив содержит собранный artforge.exe и запускает его напрямую.
Руководство по возможностям: docs/USER_GUIDE.md.
"@ | Set-Content -LiteralPath (Join-Path $Package "START_HERE_RU.md") -Encoding UTF8
    # A built portable package launches its included EXE and does not need Rust.
    @'
@echo off
setlocal
pushd "%~dp0"
if errorlevel 1 exit /b 1
"%~dp0artforge.exe" %*
set "POSEFORGE_EXIT=%errorlevel%"
if not "%POSEFORGE_EXIT%"=="0" pause
popd
exit /b %POSEFORGE_EXIT%
'@ | Set-Content -LiteralPath (Join-Path $Package "START_WINDOWS.cmd") -Encoding ASCII
    @"
ArtForge Studio $Version - portable Windows x64 package
Built: $([DateTime]::UtcNow.ToString("o"))
Target: $Target
$RustVersion
$CargoVersion

Run START_WINDOWS.cmd or artforge.exe. Rust is not required to run this package.
This package was built from the current source tree. A successful build does
not verify Windows runtime behavior, graphics drivers, GUI layout or tests.
See docs/VALIDATION.md for the scope of evidence shipped with these sources.
A compatible graphics driver is required. No GUI framework runtime is bundled.
"@ | Set-Content -LiteralPath (Join-Path $Package "BUILD.txt") -Encoding UTF8
    $Digest = (Get-FileHash -LiteralPath (Join-Path $Package "artforge.exe") -Algorithm SHA256).Hash.ToLowerInvariant()
    "$Digest  artforge.exe" | Set-Content -LiteralPath (Join-Path $Package "SHA256SUMS.txt") -Encoding ASCII
    Compress-Archive -LiteralPath $Package -DestinationPath $PendingArchive -CompressionLevel Optimal
    Move-Item -LiteralPath $PendingArchive -Destination $Archive -Force
    Write-Host "Created $Archive"
} finally {
    Pop-Location
}
