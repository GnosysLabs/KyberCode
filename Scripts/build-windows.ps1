# Runs on the Windows build host. Updater signatures are applied on the Mac
# after the installer is copied back — tauri build prompts for a key password
# over SSH and hangs.
#
# The build itself runs under cmd /c: PowerShell turns native stderr into
# NativeCommandError objects, which deadlocks over a non-TTY ssh pipe.
Set-Location C:\Users\cmcel\src\KyberCode

npm install
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

cmd /c "npx tauri build 2>&1"
exit $LASTEXITCODE
