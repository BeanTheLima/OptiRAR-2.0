# OptiRAR Automated Performance Benchmark Suite
# Reproducible, non-fabricated benchmark comparing OptiRAR vs WinRAR
param(
    [int]$SizeMB = 100,
    [string]$OutputDir = "benchmark_runs",
    [switch]$KeepArtifacts
)

$ErrorActionPreference = "Stop"

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "OptiRAR Benchmark Suite (Verification & Telemetry)" -ForegroundColor Cyan
Write-Host "Target Corpus Size: $SizeMB MB" -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

# Check binaries
$optirarCli = Resolve-Path "target\release\optirar.exe" -ErrorAction SilentlyContinue
if (-not $optirarCli) {
    $optirarCli = Resolve-Path "target\debug\optirar.exe" -ErrorAction SilentlyContinue
}
if (-not $optirarCli) {
    Write-Error "OptiRAR CLI executable not found. Run cargo build first."
    exit 1
}

$winrarExe = "C:\Program Files\WinRAR\Rar.exe"
$hasWinRar = Test-Path $winrarExe

Write-Host "OptiRAR Binary: $optirarCli" -ForegroundColor Green
if ($hasWinRar) {
    Write-Host "WinRAR Binary:  $winrarExe" -ForegroundColor Green
} else {
    Write-Host "WinRAR Binary:  Not installed (testing OptiRAR standalone)" -ForegroundColor Yellow
}

# Setup scratch directory
if (Test-Path $OutputDir) {
    Remove-Item -Path $OutputDir -Recurse -Force
}
New-Item -ItemType Directory -Path $OutputDir | Out-Null
$srcDir = "$OutputDir\source_data"
New-Item -ItemType Directory -Path $srcDir | Out-Null

Write-Host "`nGenerating $SizeMB MB mixed dataset (compressible text + binary chunks)..." -ForegroundColor Yellow

# Generate compressible text files
$textDir = "$srcDir\text_logs"
New-Item -ItemType Directory -Path $textDir | Out-Null

$sampleText = @"
2026-11-08T12:00:00.123456Z INFO [optirar_engine::extractor] Starting high performance extraction pipeline worker=0
2026-11-08T12:00:00.123489Z DEBUG [optirar_io::buffer] Allocated 256KB buffer pool slot buffer_id=14
2026-11-08T12:00:00.123512Z INFO [optirar_integrity::crc32] CRC32 hardware acceleration enabled AVX2/SSE4.2
2026-11-08T12:00:00.123545Z TRACE [optirar_index::catalog] Indexing entry path=documents/specifications.pdf size=10485760
"@
$textBytes = [System.Text.Encoding]::UTF8.GetBytes($sampleText)

for ($i = 0; $i -lt 50; $i++) {
    $filePath = "$textDir\log_$i.txt"
    $stream = [System.IO.File]::Create($filePath)
    # Write repeated text for ~500KB per file = 25MB total text
    for ($j = 0; $j -lt 2500; $j++) {
        $stream.Write($textBytes, 0, $textBytes.Length)
    }
    $stream.Close()
}

# Generate incompressible binary data files
$binDir = "$srcDir\binary_blobs"
New-Item -ItemType Directory -Path $binDir | Out-Null

$rng = New-Object System.Random(42)
$chunk = New-Object byte[] (64 * 1024) # 64KB chunk

$remainingMB = [Math]::Max(10, $SizeMB - 25)
$blobsCount = [Math]::Max(1, [int]($remainingMB / 10))

for ($b = 0; $b -lt $blobsCount; $b++) {
    $blobPath = "$binDir\blob_$b.bin"
    $stream = [System.IO.File]::Create($blobPath)
    $targetBytes = [int64](($remainingMB / $blobsCount) * 1MB)
    $written = 0
    while ($written -lt $targetBytes) {
        $rng.NextBytes($chunk)
        $toWrite = [Math]::Min($chunk.Length, $targetBytes - $written)
        $stream.Write($chunk, 0, $toWrite)
        $written += $toWrite
    }
    $stream.Close()
}

$totalGeneratedBytes = (Get-ChildItem -Path $srcDir -Recurse | Measure-Object -Property Length -Sum).Sum
$totalGeneratedMB = [Math]::Round($totalGeneratedBytes / 1MB, 2)
Write-Host "Generated dataset: $totalGeneratedMB MB across $( (Get-ChildItem -Path $srcDir -Recurse -File).Count ) files." -ForegroundColor Green

# 1. Benchmark ZIP Creation & Extraction with OptiRAR
Write-Host "`n--- [BENCHMARK 1] OptiRAR ZIP (Parallel Extraction) ---" -ForegroundColor Cyan
$zipFile = "$OutputDir\optirar_corpus.zip"
$zipOutDir = "$OutputDir\optirar_zip_extracted"

# Compression
$sw = [System.Diagnostics.Stopwatch]::StartNew()
& $optirarCli create $zipFile (Get-ChildItem -Path $srcDir).FullName --format zip --level normal
$sw.Stop()
$optirarZipCreateTime = $sw.Elapsed.TotalSeconds
$zipSizeMB = [Math]::Round((Get-Item $zipFile).Length / 1MB, 2)
Write-Host "OptiRAR ZIP Created: $zipSizeMB MB in $( [Math]::Round($optirarZipCreateTime, 2) )s"

# Extraction
$sw.Restart()
& $optirarCli extract $zipFile --output $zipOutDir --profile performance
$sw.Stop()
$optirarZipExtractTime = $sw.Elapsed.TotalSeconds
$optirarZipExtractSpeed = [Math]::Round($totalGeneratedMB / [Math]::Max(0.001, $optirarZipExtractTime), 1)
Write-Host "OptiRAR ZIP Extracted in: $( [Math]::Round($optirarZipExtractTime, 2) )s ($optirarZipExtractSpeed MB/s)" -ForegroundColor Green

# 2. Benchmark TAR Solid Extraction with OptiRAR
Write-Host "`n--- [BENCHMARK 2] OptiRAR TAR (Solid Streaming Extraction) ---" -ForegroundColor Cyan
$tarFile = "$OutputDir\optirar_corpus.tar"
$tarOutDir = "$OutputDir\optirar_tar_extracted"

$sw.Restart()
& $optirarCli create $tarFile (Get-ChildItem -Path $srcDir).FullName --format tar
$sw.Stop()
$optirarTarCreateTime = $sw.Elapsed.TotalSeconds

$sw.Restart()
& $optirarCli extract $tarFile --output $tarOutDir --profile balanced
$sw.Stop()
$optirarTarExtractTime = $sw.Elapsed.TotalSeconds
$optirarTarExtractSpeed = [Math]::Round($totalGeneratedMB / [Math]::Max(0.001, $optirarTarExtractTime), 1)
Write-Host "OptiRAR TAR Extracted in: $( [Math]::Round($optirarTarExtractTime, 2) )s ($optirarTarExtractSpeed MB/s)" -ForegroundColor Green

# 3. Benchmark WinRAR if available
$winrarResults = @{}
if ($hasWinRar) {
    Write-Host "`n--- [BENCHMARK 3] WinRAR (Rar.exe) Comparison ---" -ForegroundColor Cyan
    $winrarRarFile = "$OutputDir\winrar_corpus.rar"
    $winrarRarOut = "$OutputDir\winrar_rar_extracted"

    # WinRAR create RAR
    $sw.Restart()
    & $winrarExe a -r -m3 -y -inul $winrarRarFile "$srcDir\*"
    $sw.Stop()
    $winrarRarCreateTime = $sw.Elapsed.TotalSeconds
    $rarSizeMB = [Math]::Round((Get-Item $winrarRarFile).Length / 1MB, 2)
    Write-Host "WinRAR RAR Created: $rarSizeMB MB in $( [Math]::Round($winrarRarCreateTime, 2) )s"

    # WinRAR extract RAR
    $sw.Restart()
    & $winrarExe x -y -inul $winrarRarFile "$winrarRarOut\"
    $sw.Stop()
    $winrarRarExtractTime = $sw.Elapsed.TotalSeconds
    $winrarRarExtractSpeed = [Math]::Round($totalGeneratedMB / [Math]::Max(0.001, $winrarRarExtractTime), 1)
    Write-Host "WinRAR RAR Extracted in: $( [Math]::Round($winrarRarExtractTime, 2) )s ($winrarRarExtractSpeed MB/s)" -ForegroundColor Yellow

    # OptiRAR extract RAR created by WinRAR
    Write-Host "`n--- [BENCHMARK 4] OptiRAR Extracting WinRAR's RAR Archive ---" -ForegroundColor Cyan
    $optirarRarOut = "$OutputDir\optirar_from_winrar_extracted"
    $sw.Restart()
    & $optirarCli extract $winrarRarFile --output $optirarRarOut --profile performance
    $sw.Stop()
    $optirarRarExtractTime = $sw.Elapsed.TotalSeconds
    $optirarRarExtractSpeed = [Math]::Round($totalGeneratedMB / [Math]::Max(0.001, $optirarRarExtractTime), 1)
    Write-Host "OptiRAR extracted WinRAR's RAR in: $( [Math]::Round($optirarRarExtractTime, 2) )s ($optirarRarExtractSpeed MB/s)" -ForegroundColor Green

    $winrarResults["winrarRarExtractTime"] = $winrarRarExtractTime
    $winrarResults["winrarRarExtractSpeed"] = $winrarRarExtractSpeed
    $winrarResults["optirarRarExtractTime"] = $optirarRarExtractTime
    $winrarResults["optirarRarExtractSpeed"] = $optirarRarExtractSpeed
}

# Print Summary Table
Write-Host "`n==================================================================" -ForegroundColor Cyan
Write-Host "                     BENCHMARK RESULTS SUMMARY                     " -ForegroundColor Cyan
Write-Host "==================================================================" -ForegroundColor Cyan
Write-Host ("{0,-22} | {1,-10} | {2,-12} | {3,-15}" -f "Application / Format", "Role", "Time (s)", "Throughput (MB/s)")
Write-Host "------------------------------------------------------------------"
Write-Host ("{0,-22} | {1,-10} | {2,-12} | {3,-15}" -f "OptiRAR (ZIP)", "Extract", [Math]::Round($optirarZipExtractTime, 2), "$optirarZipExtractSpeed MB/s")
Write-Host ("{0,-22} | {1,-10} | {2,-12} | {3,-15}" -f "OptiRAR (TAR)", "Extract", [Math]::Round($optirarTarExtractTime, 2), "$optirarTarExtractSpeed MB/s")

if ($hasWinRar) {
    Write-Host ("{0,-22} | {1,-10} | {2,-12} | {3,-15}" -f "WinRAR (RAR 7.01)", "Extract", [Math]::Round($winrarResults.winrarRarExtractTime, 2), "$($winrarResults.winrarRarExtractSpeed) MB/s")
    Write-Host ("{0,-22} | {1,-10} | {2,-12} | {3,-15}" -f "OptiRAR (from RAR)", "Extract", [Math]::Round($winrarResults.optirarRarExtractTime, 2), "$($winrarResults.optirarRarExtractSpeed) MB/s")
}
Write-Host "==================================================================" -ForegroundColor Cyan

if (-not $KeepArtifacts) {
    Remove-Item -Path $OutputDir -Recurse -Force
    Write-Host "`nCleaned up benchmark scratch directory." -ForegroundColor Gray
}
