@echo off
if "%1"=="--print" if "%2"=="sysroot" (
  echo D:\python\github\OpenRhiza\OpenRhiza\tools\fake-sysroot
  exit /b 0
)
"C:\Users\eljja\.cargo\bin\rustc.exe" %*