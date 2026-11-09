# Regenerates assets\optirar.ico (16-256 px, PNG frames) and assets\icon_64.rgba (window icon).
# Design: blue rounded tile with a white archive box and a zipper.
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Definition)
$assets = Join-Path $root "assets"

function Draw-Icon([int]$n) {
    $bmp = New-Object System.Drawing.Bitmap $n, $n, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = "AntiAlias"
    $g.Clear([System.Drawing.Color]::Transparent)
    $s = $n / 64.0

    # Rounded tile with vertical gradient
    $r = 14 * $s
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $w = $n - 1
    $path.AddArc(0, 0, 2*$r, 2*$r, 180, 90); $path.AddArc($w - 2*$r, 0, 2*$r, 2*$r, 270, 90)
    $path.AddArc($w - 2*$r, $w - 2*$r, 2*$r, 2*$r, 0, 90); $path.AddArc(0, $w - 2*$r, 2*$r, 2*$r, 90, 90)
    $path.CloseFigure()
    $grad = New-Object System.Drawing.Drawing2D.LinearGradientBrush (New-Object System.Drawing.Point 0, 0), (New-Object System.Drawing.Point 0, $n), ([System.Drawing.Color]::FromArgb(255, 30, 150, 255)), ([System.Drawing.Color]::FromArgb(255, 0, 84, 170))
    $g.FillPath($grad, $path)

    # Box
    $white = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::White)
    $g.FillRectangle($white, [float](14*$s), [float](22*$s), [float](36*$s), [float](30*$s))
    # Lid
    $lid = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 220, 236, 255))
    $g.FillRectangle($lid, [float](11*$s), [float](14*$s), [float](42*$s), [float](10*$s))
    # Zipper (dark teeth down the middle)
    $dark = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 0, 70, 140))
    for ($y = 15; $y -lt 50; $y += 6) {
        $g.FillRectangle($dark, [float](29*$s), [float]($y*$s), [float](6*$s), [float]([Math]::Max(1, 3*$s)))
    }
    # Pull tab
    $g.FillEllipse($dark, [float](27*$s), [float](42*$s), [float](10*$s), [float](10*$s))
    $g.Dispose()
    return $bmp
}

$sizes = @(16, 24, 32, 48, 64, 128, 256)
$pngs = @()
foreach ($sz in $sizes) {
    $b = Draw-Icon $sz
    $ms = New-Object System.IO.MemoryStream
    if ($sz -ge 256) {
        $b.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    } else {
        # Classic DIB frame: BITMAPINFOHEADER (height x2) + bottom-up BGRA + AND mask
        $w = New-Object System.IO.BinaryWriter $ms
        $w.Write([UInt32]40); $w.Write([Int32]$sz); $w.Write([Int32]($sz*2)); $w.Write([UInt16]1); $w.Write([UInt16]32)
        $w.Write([UInt32]0); $w.Write([UInt32]0); $w.Write([Int32]0); $w.Write([Int32]0); $w.Write([UInt32]0); $w.Write([UInt32]0)
        for ($y = $sz - 1; $y -ge 0; $y--) { for ($x = 0; $x -lt $sz; $x++) {
            $c = $b.GetPixel($x, $y); $w.Write([byte]$c.B); $w.Write([byte]$c.G); $w.Write([byte]$c.R); $w.Write([byte]$c.A)
        } }
        $maskRow = [int]([Math]::Ceiling($sz / 32.0) * 4)
        $w.Write((New-Object byte[] ($maskRow * $sz)))
        $w.Flush()
    }
    $pngs += ,($ms.ToArray())
    if ($sz -eq 64) {
        # Raw RGBA (straight alpha) for eframe's window icon
        $raw = New-Object byte[] (64*64*4)
        for ($y = 0; $y -lt 64; $y++) { for ($x = 0; $x -lt 64; $x++) {
            $c = $b.GetPixel($x, $y); $i = ($y*64 + $x)*4
            $raw[$i] = $c.R; $raw[$i+1] = $c.G; $raw[$i+2] = $c.B; $raw[$i+3] = $c.A
        } }
        [IO.File]::WriteAllBytes((Join-Path $assets "icon_64.rgba"), $raw)
    }
    $b.Dispose()
}

# ICO container with PNG-compressed frames (supported since Windows Vista)
$out = New-Object System.IO.MemoryStream
$bw = New-Object System.IO.BinaryWriter $out
$bw.Write([UInt16]0); $bw.Write([UInt16]1); $bw.Write([UInt16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $sz = $sizes[$i]; $len = $pngs[$i].Length
    $dim = if ($sz -ge 256) { 0 } else { $sz }
    $bw.Write([byte]$dim); $bw.Write([byte]$dim); $bw.Write([byte]0); $bw.Write([byte]0)
    $bw.Write([UInt16]1); $bw.Write([UInt16]32); $bw.Write([UInt32]$len); $bw.Write([UInt32]$offset)
    $offset += $len
}
foreach ($p in $pngs) { $bw.Write($p) }
$bw.Flush()
[IO.File]::WriteAllBytes((Join-Path $assets "optirar.ico"), $out.ToArray())
Write-Host "Wrote assets\optirar.ico and assets\icon_64.rgba"
