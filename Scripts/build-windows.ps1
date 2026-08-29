# Runs on the Windows build host. Expects the updater signing key at
# C:\Users\cmcel\_kyber_signing.key (placed by build-windows.sh, deleted after).
#
# The build itself runs under cmd /c: PowerShell turns native stderr into
# NativeCommandError objects, which deadlocks over a non-TTY ssh pipe.
Set-Location C:\Users\cmcel\src\KyberCode

npm install
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content C:\Users\cmcel\_kyber_signing.key -Raw).TrimEnd()
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""

cmd /c "npx tauri build 2>&1"
exit $LASTEXITCODE
