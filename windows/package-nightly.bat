@echo off
setlocal

rem Define the file where you want to replace the text
set "file=.\nightly.nsi"
set "oldText=/Git_Commit_Hash/g"
set "newText=COMMIT_HASH"

rem Get the latest commit hash
for /f "delims=" %%i in ('git rev-parse --short HEAD') do set "newText=nightly-%%i"
rem Get the package version
for /f "tokens=2 delims=@ " %%i in ('cargo pkgid') do set "package_version=%%i"

rem Check if the file exists
if exist "%file%" (
    rem Use PowerShell to replace the text
    powershell -Command "(Get-Content '%file%') -replace '%oldText%', 'v%package_version%-%newText%' | Set-Content '%file%'"
    echo Replaced all occurrences of %oldText% with v%package_version%-%newText% in %file%.
) else (
    echo File not found: %file%
)

mkdir windows\output
makensis %file%

endlocal