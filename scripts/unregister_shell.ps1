# OptiRAR - remove Windows Explorer integration (HKCU)

Write-Host "Unregistering OptiRAR shell integration..." -ForegroundColor Cyan

$ArchiveExts = @(".zip", ".rar", ".7z", ".tar", ".gz", ".tgz", ".zst", ".tzst", ".cbz", ".cbr", ".cb7")
$ProgId = "OptiRAR.Archive"
$HKCU = [Microsoft.Win32.Registry]::CurrentUser
$backup = $HKCU.OpenSubKey("Software\OptiRAR\Backup")

foreach ($ext in $ArchiveExts) {
    try { $HKCU.DeleteSubKeyTree("Software\Classes\SystemFileAssociations\$ext\shell\OptiRAR", $false) } catch {}
    try {
        $k = $HKCU.OpenSubKey("Software\Classes\$ext\OpenWithProgids", $true)
        if ($k) { $k.DeleteValue($ProgId, $false); $k.Close() }
    } catch {}
    try {
        # Restore the previous default program if OptiRAR had taken it over.
        $k = $HKCU.OpenSubKey("Software\Classes\$ext", $true)
        if ($k -and $k.GetValue("") -eq $ProgId) {
            $old = if ($backup) { $backup.GetValue($ext) } else { $null }
            if ($old) { $k.SetValue("", $old) } else { $k.DeleteValue("", $false) }
        }
        if ($k) { $k.Close() }
    } catch {}
}
if ($backup) { $backup.Close() }

foreach ($path in @(
    "Software\Classes\*\shell\OptiRAR",
    "Software\Classes\Directory\shell\OptiRAR",
    "Software\Classes\$ProgId",
    "Software\Classes\Applications\optirar-desktop.exe",
    "Software\OptiRAR")) {
    try { $HKCU.DeleteSubKeyTree($path, $false) } catch {}
}
try {
    $k = $HKCU.OpenSubKey("Software\RegisteredApplications", $true)
    if ($k) { $k.DeleteValue("OptiRAR", $false); $k.Close() }
} catch {}

try {
    Add-Type -Namespace OptiRar -Name Shell -MemberDefinition '[DllImport("shell32.dll")] public static extern void SHChangeNotify(int e, uint f, IntPtr a, IntPtr b);'
    [OptiRar.Shell]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)
} catch {}

Write-Host "OptiRAR shell integration removed." -ForegroundColor Green
