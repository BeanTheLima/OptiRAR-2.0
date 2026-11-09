# OptiRAR Setup - per-user install (no admin), WinRAR-style:
#   copies OptiRAR to %LOCALAPPDATA%\Programs\OptiRAR, registers Explorer menus and file
#   associations, creates Start menu / Desktop / "Send to" shortcuts and an uninstaller entry.

param(
    [string]$InstallDir = "$env:LOCALAPPDATA\Programs\OptiRAR",
    [switch]$NoDesktopShortcut,
    [switch]$NoDefaultAssociation
)

$ErrorActionPreference = "Stop"
$Src = Split-Path -Parent $MyInvocation.MyCommand.Definition
$vi = (Get-UICulture).Name -like "vi*"
function L($en, $viText) { if ($vi) { $viText } else { $en } }

if (-not (Test-Path "$Src\optirar-desktop.exe")) {
    Write-Error "optirar-desktop.exe must be next to install.ps1"
    exit 1
}

Write-Host (L "Installing OptiRAR to $InstallDir ..." "Đang cài OptiRAR vào $InstallDir ...") -ForegroundColor Cyan

# Close running copies so the files can be replaced.
Get-Process optirar-desktop -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$InstallDir*" } | Stop-Process -Force -ErrorAction SilentlyContinue

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
foreach ($f in @("optirar-desktop.exe", "optirar.exe", "register_shell.ps1", "unregister_shell.ps1", "uninstall.ps1", "README.md", "LICENSE")) {
    if (Test-Path "$Src\$f") { Copy-Item "$Src\$f" -Destination $InstallDir -Force }
}
$Gui = Join-Path $InstallDir "optirar-desktop.exe"

# Explorer integration + associations
$lang = if ($vi) { "vi" } else { "en" }
& "$InstallDir\register_shell.ps1" -OptiRarGui $Gui -Lang $lang -SetDefault:(-not $NoDefaultAssociation)

# Shortcuts
$shell = New-Object -ComObject WScript.Shell
function New-Shortcut($path, $arguments = "", $desc = "OptiRAR") {
    $s = $shell.CreateShortcut($path)
    $s.TargetPath = $Gui
    $s.Arguments = $arguments
    $s.WorkingDirectory = $InstallDir
    $s.IconLocation = "$Gui,0"
    $s.Description = $desc
    $s.Save()
}
$programs = [Environment]::GetFolderPath("Programs")
New-Shortcut (Join-Path $programs "OptiRAR.lnk")
if (-not $NoDesktopShortcut) { New-Shortcut (Join-Path ([Environment]::GetFolderPath("Desktop")) "OptiRAR.lnk") }
# "Send to" handles many selected files in ONE archive (static context-menu verbs run once per file).
New-Shortcut (Join-Path ([Environment]::GetFolderPath("SendTo")) (L "OptiRAR (compress to ZIP).lnk" "OptiRAR (nén thành ZIP).lnk")) "--add-zip"
New-Shortcut (Join-Path ([Environment]::GetFolderPath("SendTo")) (L "OptiRAR (extract each to its folder).lnk" "OptiRAR (giải nén từng tệp vào thư mục riêng).lnk")) "--extract-folder"

# Apps & features entry
$size = [int]((Get-ChildItem $InstallDir -Recurse | Measure-Object Length -Sum).Sum / 1KB)
$u = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey("Software\Microsoft\Windows\CurrentVersion\Uninstall\OptiRAR")
$u.SetValue("DisplayName", "OptiRAR")
$u.SetValue("DisplayVersion", "2.0.0")
$u.SetValue("Publisher", "OptiRAR")
$u.SetValue("DisplayIcon", "$Gui,0")
$u.SetValue("InstallLocation", $InstallDir)
$u.SetValue("UninstallString", "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$InstallDir\uninstall.ps1`"")
$u.SetValue("NoModify", 1, [Microsoft.Win32.RegistryValueKind]::DWord)
$u.SetValue("NoRepair", 1, [Microsoft.Win32.RegistryValueKind]::DWord)
$u.SetValue("EstimatedSize", $size, [Microsoft.Win32.RegistryValueKind]::DWord)
$u.Close()

Write-Host (L "OptiRAR installed. Right-click any archive -> OptiRAR (Windows 11: Show more options)." "Đã cài OptiRAR. Chuột phải vào tệp nén -> OptiRAR (Windows 11: Hiển thị thêm tùy chọn).") -ForegroundColor Green
