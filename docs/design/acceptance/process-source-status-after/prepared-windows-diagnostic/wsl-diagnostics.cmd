@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64 > "C:\Users\han\AppData\Local\Orchester-build\activity-source-audit-20261110\diagnostic-vs-env.log" 2>&1
if errorlevel 1 exit /b %errorlevel%
cd /d "C:\Users\han\AppData\Local\Orchester-build\activity-source-audit-20261110"
rustc --edition 2021 --target x86_64-pc-windows-msvc wsl-diagnostics.rs -o wsl-diagnostics.exe > diagnostic-build.log 2>&1
if errorlevel 1 exit /b %errorlevel%
wsl-diagnostics.exe > wsl-handle-diagnostics.json
exit /b %errorlevel%
