@echo off
setlocal
chcp 65001 >nul
pushd "%~dp0"
if errorlevel 1 exit /b 1
where cargo >nul 2>nul
if errorlevel 1 goto missing_rust

echo ArtForge Studio 0.48.3: сборка обновлённых исходников.
echo Требуются Rust 1.99.1, C++ Build Tools и доступ к зависимостям при первой сборке.
echo Указанная версия Rust выбирается через rust-toolchain.toml.
cargo build --release --locked
if errorlevel 1 goto build_failed
if not exist "target\release\artforge.exe" goto missing_binary
"target\release\artforge.exe" %*
set "POSEFORGE_EXIT=%errorlevel%"
if not "%POSEFORGE_EXIT%"=="0" pause
popd
exit /b %POSEFORGE_EXIT%

:missing_rust
echo Не найден cargo. Установите Rust 1.99.1 через rustup и C++ Build Tools для Windows x64.
echo После установки откройте этот файл заново. Подробности: START_HERE_RU.md.
goto failed

:build_failed
echo Сборка ArtForge Studio 0.48.3 не завершена. Исправьте ошибку выше и повторите запуск.
goto failed

:missing_binary
echo После сборки не найден target\release\artforge.exe.
echo Проверьте CARGO_TARGET_DIR и CARGO_BUILD_TARGET: этот запуск ожидает обычную Windows-сборку.

:failed
pause
popd
exit /b 1
