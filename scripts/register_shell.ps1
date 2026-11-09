# OptiRAR - Windows Explorer integration (per-user, HKCU, no admin needed)
#  * "OptiRAR" submenu on archives:  Open / Extract files... / Extract here / Extract to "<name>\" / Test
#  * "OptiRAR" submenu on any file or folder: Add to archive... / Add to ZIP
#  * Registers OptiRAR.Archive ProgID, "Open with" entries and Default Apps capabilities
#  * -SetDefault makes OptiRAR the default program for archives (like WinRAR)

param(
    [string]$OptiRarGui = "",
    [string]$OptiRarCli = "",
    [string]$Lang = "",
    [switch]$SetDefault
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition

function Find-Exe($name) {
    foreach ($dir in @($ScriptDir, (Split-Path -Parent $ScriptDir), "$PWD", "$PWD\dist",
                       (Join-Path (Split-Path -Parent $ScriptDir) "dist"),
                       (Join-Path (Split-Path -Parent $ScriptDir) "target\release"))) {
        $p = Join-Path $dir $name
        if (Test-Path $p) { return (Resolve-Path $p).Path }
    }
    $cmd = Get-Command $name -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    return ""
}

if (-not $OptiRarGui) { $OptiRarGui = Find-Exe "optirar-desktop.exe" }
if (-not $OptiRarGui -or -not (Test-Path $OptiRarGui)) {
    Write-Error "optirar-desktop.exe not found. Pass -OptiRarGui <path>."
    exit 1
}
$OptiRarGui = (Resolve-Path $OptiRarGui).Path

if (-not $Lang) { $Lang = if ((Get-UICulture).Name -like "vi*") { "vi" } else { "en" } }
$vi = $Lang -like "vi*"
function L($en, $viText) { if ($vi) { $viText } else { $en } }

Write-Host "Registering OptiRAR shell integration..." -ForegroundColor Cyan
Write-Host "  GUI: $OptiRarGui"

$ArchiveExts = @(".zip", ".rar", ".7z", ".tar", ".gz", ".tgz", ".zst", ".tzst", ".cbz", ".cbr", ".cb7")
$ProgId = "OptiRAR.Archive"
$Exe = "`"$OptiRarGui`""
$HKCU = [Microsoft.Win32.Registry]::CurrentUser

function Set-RegKey($subPath, $defaultVal, $props = @{}) {
    $key = $HKCU.CreateSubKey($subPath)
    if ($null -ne $defaultVal) { $key.SetValue("", $defaultVal) }
    foreach ($p in $props.Keys) { $key.SetValue($p, $props[$p]) }
    $key.Close()
}

function Add-Verb($root, $id, $label, $command, $icon = $null) {
    # Player: verb stays visible for any number of selected items (default hides it above 15).
    $props = @{ "MUIVerb" = $label; "MultiSelectModel" = "Player" }
    if ($icon) { $props["Icon"] = $icon }
    Set-RegKey "$root\shell\$id" $null $props
    Set-RegKey "$root\shell\$id\command" $command
}

# Remove previous versions first so stale verbs (old CLI commands) disappear.
foreach ($ext in $ArchiveExts) {
    try { $HKCU.DeleteSubKeyTree("Software\Classes\SystemFileAssociations\$ext\shell\OptiRAR", $false) } catch {}
}
foreach ($r in @("Software\Classes\*\shell\OptiRAR", "Software\Classes\Directory\shell\OptiRAR")) {
    try { $HKCU.DeleteSubKeyTree($r, $false) } catch {}
}

# 1. ProgID used for double-click / "Open with"
Set-RegKey "Software\Classes\$ProgId" (L "OptiRAR archive" "Tệp nén OptiRAR") @{ "FriendlyTypeName" = (L "OptiRAR archive" "Tệp nén OptiRAR") }
Set-RegKey "Software\Classes\$ProgId\DefaultIcon" "$OptiRarGui,0"
Set-RegKey "Software\Classes\$ProgId\shell" "open"
Set-RegKey "Software\Classes\$ProgId\shell\open" (L "Open with OptiRAR" "Mở bằng OptiRAR")
Set-RegKey "Software\Classes\$ProgId\shell\open\command" "$Exe `"%1`""

# 2. Applications entry (shows OptiRAR in "Open with" -> "Choose another app")
$app = "Software\Classes\Applications\optirar-desktop.exe"
Set-RegKey $app $null @{ "FriendlyAppName" = "OptiRAR" }
Set-RegKey "$app\shell\open\command" "$Exe `"%1`""
foreach ($ext in $ArchiveExts) { Set-RegKey "$app\SupportedTypes" $null @{ $ext = "" } }

# 3. Default Apps capabilities
Set-RegKey "Software\OptiRAR\Capabilities" $null @{
    "ApplicationName" = "OptiRAR"
    "ApplicationDescription" = (L "Fast archive manager" "Trình quản lý tệp nén tốc độ cao")
}
Set-RegKey "Software\RegisteredApplications" $null @{ "OptiRAR" = "Software\OptiRAR\Capabilities" }

foreach ($ext in $ArchiveExts) {
    Set-RegKey "Software\OptiRAR\Capabilities\FileAssociations" $null @{ $ext = $ProgId }
    Set-RegKey "Software\Classes\$ext\OpenWithProgids" $null @{ $ProgId = "" }

    if ($SetDefault) {
        $k = $HKCU.CreateSubKey("Software\Classes\$ext")
        $old = $k.GetValue("")
        if ($old -and $old -ne $ProgId) { Set-RegKey "Software\OptiRAR\Backup" $null @{ $ext = $old } }
        $k.SetValue("", $ProgId)
        $k.Close()
    }

    # 4. Archive context menu (cascading, like WinRAR)
    $root = "Software\Classes\SystemFileAssociations\$ext\shell\OptiRAR"
    Set-RegKey $root $null @{ "MUIVerb" = "OptiRAR"; "Icon" = "$OptiRarGui,0"; "SubCommands" = "" }
    Add-Verb $root "01_Open"        (L "Open with OptiRAR" "Mở bằng OptiRAR")                 "$Exe `"%1`"" "$OptiRarGui,0"
    Add-Verb $root "02_ExtractTo"   (L "Extract files..." "Giải nén tệp...")                   "$Exe --extract-dialog `"%1`""
    Add-Verb $root "03_ExtractHere" (L "Extract Here" "Giải nén tại đây")                      "$Exe --extract-here `"%1`""
    Add-Verb $root "04_ExtractDir"  (L "Extract to subfolder (archive name)" "Giải nén vào thư mục (tên tệp nén)") "$Exe --extract-folder `"%1`""
    Add-Verb $root "05_Test"        (L "Test archive" "Kiểm tra tệp nén")                      "$Exe --test `"%1`""
}

# 5. Any file / folder: add to archive
foreach ($root in @("Software\Classes\*\shell\OptiRAR", "Software\Classes\Directory\shell\OptiRAR")) {
    Set-RegKey $root $null @{ "MUIVerb" = "OptiRAR"; "Icon" = "$OptiRarGui,0"; "SubCommands" = "" }
    Add-Verb $root "01_AddDialog" (L "Add to archive..." "Thêm vào tệp nén...") "$Exe --create `"%1`"" "$OptiRarGui,0"
    Add-Verb $root "02_AddZip"    (L "Add to ZIP archive (same name)" "Nén thành tệp ZIP (cùng tên)") "$Exe --add-zip `"%1`""
}

# Tell Explorer that associations changed.
try {
    Add-Type -Namespace OptiRar -Name Shell -MemberDefinition '[DllImport("shell32.dll")] public static extern void SHChangeNotify(int e, uint f, IntPtr a, IntPtr b);'
    [OptiRar.Shell]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)
} catch {}

Write-Host "OptiRAR shell integration registered (HKCU)." -ForegroundColor Green
