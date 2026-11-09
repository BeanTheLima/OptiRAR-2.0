# OptiRAR uninstaller (per-user)

$InstallDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$vi = (Get-UICulture).Name -like "vi*"

Get-Process optirar-desktop -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$InstallDir*" } | Stop-Process -Force -ErrorAction SilentlyContinue

if (Test-Path "$InstallDir\unregister_shell.ps1") { & "$InstallDir\unregister_shell.ps1" }

foreach ($lnk in @(
    (Join-Path ([Environment]::GetFolderPath("Programs")) "OptiRAR.lnk"),
    (Join-Path ([Environment]::GetFolderPath("Desktop")) "OptiRAR.lnk"))) {
    Remove-Item $lnk -Force -ErrorAction SilentlyContinue
}
Get-ChildItem ([Environment]::GetFolderPath("SendTo")) -Filter "OptiRAR*.lnk" -ErrorAction SilentlyContinue | Remove-Item -Force

try { [Microsoft.Win32.Registry]::CurrentUser.DeleteSubKeyTree("Software\Microsoft\Windows\CurrentVersion\Uninstall\OptiRAR", $false) } catch {}

# The folder holds this running script: delete it from a detached cmd after we exit.
if ($InstallDir -like "*\Programs\OptiRAR") {
    Start-Process cmd.exe -ArgumentList "/c timeout /t 2 /nobreak >nul & rmdir /s /q `"$InstallDir`"" -WindowStyle Hidden
}

Write-Host $(if ($vi) { "Đã gỡ OptiRAR." } else { "OptiRAR uninstalled." }) -ForegroundColor Green
