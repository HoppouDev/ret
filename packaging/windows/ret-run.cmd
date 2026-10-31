@echo off
setlocal
if not defined RET_APK set "RET_APK=%~dp0ret.apk"
"%~dp0ret.exe" run
set "ret_exit=%errorlevel%"
pause
exit /b %ret_exit%
