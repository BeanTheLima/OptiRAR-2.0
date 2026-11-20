# OptiRAR Automated Comprehensive Performance Benchmark Suite
# Verifies extraction throughput across RAR, ZIP, 7z, and tests .orr archive creation and extraction.
param(
    [int]$SizeMB = 100,
    [string]$OutputDir = "benchmark_runs",
    [switch]$KeepArtifacts
)

$ErrorActionPreference = "Stop"

Write-Host "==================================================================" -ForegroundColor Cyan
Write-Host "OptiRAR Comprehensive Benchmark & Verification Suite" -ForegroundColor Cyan
Write-Host "Target Corpus Size: $SizeMB MB" -ForegroundColor Cyan
Write-Host "==================================================================" -ForegroundColor Cyan

# Check binaries
$optirarCli = Resolve-Path "target\release\optirar.exe" -ErrorAction SilentlyContinue
if (-not $optirarCli) {
    $optirarCli = Resolve-Path "target\debug\optirar.exe" -ErrorAction SilentlyContinue
}
if (-not $optirarCli) {
    Write-Error "OptiRAR CLI executable not found. Build target\release\optirar.exe first."
    exit 1
}

$winrarExe = "C:\Program Files\WinRAR\Rar.exe"
$hasWinRar = Test-Path $winrarExe

Write-Host "OptiRAR Binary: $optirarCli" -ForegroundColor Green
if ($hasWinRar) {
    Write-Host "WinRAR Binary:  $winrarExe" -ForegroundColor Green
} else {
    Write-Host "WinRAR Binary:  Not installed" -ForegroundColor Yellow
}

# Setup scratch directory (absolute paths)
$OutputDir = [System.IO.Path]::GetFullPath($OutputDir)
if (Test-Path $OutputDir) {
    Remove-Item -Path $OutputDir -Recurse -Force
}
New-Item -ItemType Directory -Path $OutputDir | Out-Null
$srcDir = [System.IO.Path]::GetFullPath((New-Item -ItemType Directory -Path "$OutputDir\source_data").FullName)

Write-Host "`nGenerating $SizeMB MB mixed dataset (structured text, JSON, and binary chunks)..." -ForegroundColor Yellow

# 1. Text / Logs
$textDir = "$srcDir\text_logs"
New-Item -ItemType Directory -Path $textDir | Out-Null
$sampleLog = @"
2026-11-20T08:15:22.104Z [INFO] optirar_engine::extractor: Spawning parallel Rayon worker pool threads=8
2026-11-20T08:15:22.105Z [DEBUG] optirar_io::writer: Preallocating NTFS contiguous disk extents size=10485760
2026-11-20T08:15:22.106Z [INFO] optirar_formats::orr_format: Decompressing 4MB Zstd chunk 0 hash_verified=true
2026-11-20T08:15:22.107Z [TRACE] optirar_integrity::blake3: Block checksum match BLAKE3=e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
"@
$logBytes = [System.Text.Encoding]::UTF8.GetBytes($sampleLog)
$textFilesCount = 40
$targetPerTextFile = [int]((($SizeMB * 0.40) / $textFilesCount) * 1MB)
for ($i = 0; $i -lt $textFilesCount; $i++) {
    $filePath = "$textDir\server_log_$i.log"
    $stream = [System.IO.File]::Create($filePath)
    $written = 0
    while ($written -lt $targetPerTextFile) {
        $stream.Write($logBytes, 0, $logBytes.Length)
        $written += $logBytes.Length
    }
    $stream.Close()
}

# 2. JSON Structured Data
$jsonDir = "$srcDir\json_records"
New-Item -ItemType Directory -Path $jsonDir | Out-Null
$sampleJson = '{"id":1001,"status":"active","metrics":{"cpu_usage":4.2,"ram_mb":18.4,"throughput":1450.2},"tags":["rust","optirar","high_speed","nvme_optimized"]}' + "`n"
$jsonBytes = [System.Text.Encoding]::UTF8.GetBytes($sampleJson)
$jsonFilesCount = 20
$targetPerJsonFile = [int]((($SizeMB * 0.35) / $jsonFilesCount) * 1MB)
for ($i = 0; $i -lt $jsonFilesCount; $i++) {
    $filePath = "$jsonDir\records_$i.json"
    $stream = [System.IO.File]::Create($filePath)
    $written = 0
    while ($written -lt $targetPerJsonFile) {
        $stream.Write($jsonBytes, 0, $jsonBytes.Length)
        $written += $jsonBytes.Length
    }
    $stream.Close()
}

# 3. Binary Blobs (Moderate Entropy)
$binDir = "$srcDir\binary_blobs"
New-Item -ItemType Directory -Path $binDir | Out-Null
$rng = New-Object System.Random(42)
$chunk = New-Object byte[] (64 * 1024)
$binFilesCount = 10
$targetPerBinFile = [int]((($SizeMB * 0.25) / $binFilesCount) * 1MB)
for ($b = 0; $b -lt $binFilesCount; $b++) {
    $blobPath = "$binDir\blob_$b.bin"
    $stream = [System.IO.File]::Create($blobPath)
    $written = 0
    while ($written -lt $targetPerBinFile) {
        $rng.NextBytes($chunk)
        $toWrite = [Math]::Min($chunk.Length, $targetPerBinFile - $written)
        $stream.Write($chunk, 0, $toWrite)
        $written += $toWrite
    }
    $stream.Close()
}

$totalGeneratedBytes = (Get-ChildItem -Path $srcDir -Recurse -File | Measure-Object -Property Length -Sum).Sum
$totalGeneratedMB = [Math]::Round($totalGeneratedBytes / 1MB, 2)
$totalFileCount = (Get-ChildItem -Path $srcDir -Recurse -File).Count
Write-Host "Generated dataset: $totalGeneratedMB MB across $totalFileCount files." -ForegroundColor Green

# Compute original dataset hash for integrity verification
Write-Host "Computing baseline integrity hashes..." -ForegroundColor Gray
$sourceFiles = Get-ChildItem -Path $srcDir -Recurse -File | Sort-Object FullName
$sourceHashes = @{}
foreach ($sf in $sourceFiles) {
    $rel = $sf.FullName.Substring($srcDir.Length).TrimStart('\', '/').Replace('\', '/')
    $sourceHashes[$rel] = (Get-FileHash -Path $sf.FullName -Algorithm MD5).Hash
}

# Helper to verify extracted files
function Verify-ExtractedDir($dirPath) {
    $fullDirPath = [System.IO.Path]::GetFullPath($dirPath)
    $extractedFiles = Get-ChildItem -Path $fullDirPath -Recurse -File -ErrorAction SilentlyContinue
    if (-not $extractedFiles -or $extractedFiles.Count -eq 0) {
        return @{ Success = $false; MatchCount = 0; Error = "No files extracted" }
    }
    $matched = 0
    foreach ($ef in $extractedFiles) {
        $rel = $ef.FullName.Substring($fullDirPath.Length).TrimStart('\', '/').Replace('\', '/')
        $cleanRel = $rel
        if (-not $sourceHashes.ContainsKey($cleanRel)) {
            $parts = $rel -split '[\\/]', 2
            if ($parts.Length -gt 1 -and $sourceHashes.ContainsKey($parts[1])) {
                $cleanRel = $parts[1]
            }
        }
        if ($sourceHashes.ContainsKey($cleanRel)) {
            $hash = (Get-FileHash -Path $ef.FullName -Algorithm MD5).Hash
            if ($hash -ne $sourceHashes[$cleanRel]) {
                return @{ Success = $false; MatchCount = $matched; Error = "Hash mismatch on $cleanRel" }
            }
            $matched++
        }
    }
    return @{ Success = ($matched -eq $sourceFiles.Count); MatchCount = $matched; Error = $null }
}

$results = [System.Collections.Generic.List[PSCustomObject]]::new()
$sw = [System.Diagnostics.Stopwatch]::new()

# ====================================================================
# 1. ZIP BENCHMARK
# ====================================================================
Write-Host "`n--- [BENCHMARK 1] ZIP Format ---" -ForegroundColor Cyan
$zipFile = [System.IO.Path]::GetFullPath("$OutputDir\corpus.zip")
$zipOutDir = [System.IO.Path]::GetFullPath("$OutputDir\zip_extracted")

$sw.Restart()
& $optirarCli create $zipFile (Get-ChildItem -Path $srcDir).FullName
$sw.Stop()
$zipCreateTime = $sw.Elapsed.TotalSeconds
$zipSizeMB = [Math]::Round((Get-Item $zipFile).Length / 1MB, 2)
$zipCreateSpeed = [Math]::Round($totalGeneratedMB / [Math]::Max(0.001, $zipCreateTime), 1)
$zipRatio = [Math]::Round(($zipSizeMB / $totalGeneratedMB) * 100, 1)

$sw.Restart()
& $optirarCli extract $zipFile --output $zipOutDir --profile performance
$sw.Stop()
$zipExtractTime = $sw.Elapsed.TotalSeconds
$zipExtractSpeed = [Math]::Round($totalGeneratedMB / [Math]::Max(0.001, $zipExtractTime), 1)
$zipVerify = Verify-ExtractedDir $zipOutDir

$zipBenchJson = & $optirarCli benchmark $zipFile --json | ConvertFrom-Json
$zipRawDecompressSpeed = [Math]::Round($zipBenchJson.throughput_mb_s, 1)

$zipColor = if ($zipVerify.Success) { "Green" } else { "Red" }
Write-Host "ZIP Created:   $zipSizeMB MB ($zipRatio%) in $([Math]::Round($zipCreateTime, 2))s ($zipCreateSpeed MB/s)"
Write-Host "ZIP Extracted: in $([Math]::Round($zipExtractTime, 2))s ($zipExtractSpeed MB/s to disk | Raw Decompress: $zipRawDecompressSpeed MB/s)" -ForegroundColor Green
Write-Host "ZIP Integrity: $($zipVerify.Success) ($($zipVerify.MatchCount)/$($sourceFiles.Count) verified)" -ForegroundColor $zipColor

$results.Add([PSCustomObject]@{
    Format = "ZIP"
    Engine = "OptiRAR"
    SizeMB = $zipSizeMB
    RatioPct = $zipRatio
    CreateTimeSec = [Math]::Round($zipCreateTime, 2)
    CreateSpeedMbs = $zipCreateSpeed
    ExtractTimeSec = [Math]::Round($zipExtractTime, 2)
    DiskExtractMbs = $zipExtractSpeed
    RawDecompressMbs = $zipRawDecompressSpeed
    Integrity = $(if ($zipVerify.Success) { "100% Pass" } else { "Fail" })
})

# ====================================================================
# 2. 7-ZIP (7z) BENCHMARK
# ====================================================================
Write-Host "`n--- [BENCHMARK 2] 7-Zip (7z) Format ---" -ForegroundColor Cyan
$sevenzFile = [System.IO.Path]::GetFullPath("$OutputDir\corpus.7z")
$sevenzOutDir = [System.IO.Path]::GetFullPath("$OutputDir\sevenz_extracted")

$sw.Restart()
& $optirarCli create $sevenzFile (Get-ChildItem -Path $srcDir).FullName
$sw.Stop()
$sevenzCreateTime = $sw.Elapsed.TotalSeconds
$sevenzSizeMB = [Math]::Round((Get-Item $sevenzFile).Length / 1MB, 2)
$sevenzCreateSpeed = [Math]::Round($totalGeneratedMB / [Math]::Max(0.001, $sevenzCreateTime), 1)
$sevenzRatio = [Math]::Round(($sevenzSizeMB / $totalGeneratedMB) * 100, 1)

$sw.Restart()
& $optirarCli extract $sevenzFile --output $sevenzOutDir --profile performance
$sw.Stop()
$sevenzExtractTime = $sw.Elapsed.TotalSeconds
$sevenzExtractSpeed = [Math]::Round($totalGeneratedMB / [Math]::Max(0.001, $sevenzExtractTime), 1)
$sevenzVerify = Verify-ExtractedDir $sevenzOutDir

$sevenzBenchJson = & $optirarCli benchmark $sevenzFile --json | ConvertFrom-Json
$sevenzRawDecompressSpeed = [Math]::Round($sevenzBenchJson.throughput_mb_s, 1)

$sevenzColor = if ($sevenzVerify.Success) { "Green" } else { "Red" }
Write-Host "7z Created:   $sevenzSizeMB MB ($sevenzRatio%) in $([Math]::Round($sevenzCreateTime, 2))s ($sevenzCreateSpeed MB/s)"
Write-Host "7z Extracted: in $([Math]::Round($sevenzExtractTime, 2))s ($sevenzExtractSpeed MB/s to disk | Raw Decompress: $sevenzRawDecompressSpeed MB/s)" -ForegroundColor Green
Write-Host "7z Integrity: $($sevenzVerify.Success) ($($sevenzVerify.MatchCount)/$($sourceFiles.Count) verified)" -ForegroundColor $sevenzColor

$results.Add([PSCustomObject]@{
    Format = "7-Zip (7z)"
    Engine = "OptiRAR"
    SizeMB = $sevenzSizeMB
    RatioPct = $sevenzRatio
    CreateTimeSec = [Math]::Round($sevenzCreateTime, 2)
    CreateSpeedMbs = $sevenzCreateSpeed
    ExtractTimeSec = [Math]::Round($sevenzExtractTime, 2)
    DiskExtractMbs = $sevenzExtractSpeed
    RawDecompressMbs = $sevenzRawDecompressSpeed
    Integrity = $(if ($sevenzVerify.Success) { "100% Pass" } else { "Fail" })
})

# ====================================================================
# 3. RAR FORMAT BENCHMARK (Non-Solid & Solid, WinRAR vs OptiRAR)
# ====================================================================
if ($hasWinRar) {
    Write-Host "`n--- [BENCHMARK 3] RAR Format (Non-Solid - Multi-Thread Parallel) ---" -ForegroundColor Cyan
    $rarNonSolidFile = [System.IO.Path]::GetFullPath("$OutputDir\corpus_nonsolid.rar")
    $winrarNsOutDir = [System.IO.Path]::GetFullPath("$OutputDir\winrar_rar_nonsolid_extracted")
    $optirarNsOutDir = [System.IO.Path]::GetFullPath("$OutputDir\optirar_rar_nonsolid_extracted")

    # Create Non-Solid RAR using WinRAR (-s-)
    Push-Location $srcDir
    & $winrarExe a -r -m3 -s- -y -inul "$rarNonSolidFile" *
    Pop-Location
    $rarNsSizeMB = [Math]::Round((Get-Item $rarNonSolidFile).Length / 1MB, 2)
    $rarNsRatio = [Math]::Round(($rarNsSizeMB / $totalGeneratedMB) * 100, 1)

    # WinRAR extract Non-Solid RAR
    $sw.Restart()
    & $winrarExe x -y -inul $rarNonSolidFile "$winrarNsOutDir\"
    $sw.Stop()
    $winrarNsExtractTime = $sw.Elapsed.TotalSeconds
    $winrarNsExtractSpeed = [Math]::Round($totalGeneratedMB / [Math]::Max(0.001, $winrarNsExtractTime), 1)

    # OptiRAR extract Non-Solid RAR (Parallel try_clone execution!)
    $sw.Restart()
    & $optirarCli extract $rarNonSolidFile --output $optirarNsOutDir --profile performance
    $sw.Stop()
    $optirarNsExtractTime = $sw.Elapsed.TotalSeconds
    $optirarNsExtractSpeed = [Math]::Round($totalGeneratedMB / [Math]::Max(0.001, $optirarNsExtractTime), 1)
    $optirarNsVerify = Verify-ExtractedDir $optirarNsOutDir

    $rarNsBenchJson = & $optirarCli benchmark $rarNonSolidFile --json | ConvertFrom-Json
    $rarNsRawDecompressSpeed = [Math]::Round($rarNsBenchJson.throughput_mb_s, 1)

    $rarNsColor = if ($optirarNsVerify.Success) { "Green" } else { "Red" }
    Write-Host "Non-Solid RAR Size: $rarNsSizeMB MB ($rarNsRatio%)"
    Write-Host "WinRAR (Rar.exe) Extract: in $([Math]::Round($winrarNsExtractTime, 2))s ($winrarNsExtractSpeed MB/s)" -ForegroundColor Yellow
    Write-Host "OptiRAR Extract (Parallel): in $([Math]::Round($optirarNsExtractTime, 2))s ($optirarNsExtractSpeed MB/s to disk | Raw: $rarNsRawDecompressSpeed MB/s)" -ForegroundColor Green
    Write-Host "OptiRAR RAR Integrity: $($optirarNsVerify.Success) ($($optirarNsVerify.MatchCount)/$($sourceFiles.Count) verified)" -ForegroundColor $rarNsColor

    $results.Add([PSCustomObject]@{
        Format = "RAR (Non-Solid)"
        Engine = "WinRAR 7.01"
        SizeMB = $rarNsSizeMB
        RatioPct = $rarNsRatio
        CreateTimeSec = "-"
        CreateSpeedMbs = "-"
        ExtractTimeSec = [Math]::Round($winrarNsExtractTime, 2)
        DiskExtractMbs = $winrarNsExtractSpeed
        RawDecompressMbs = "-"
        Integrity = "Baseline"
    })
    $results.Add([PSCustomObject]@{
        Format = "RAR (Non-Solid)"
        Engine = "OptiRAR"
        SizeMB = $rarNsSizeMB
        RatioPct = $rarNsRatio
        CreateTimeSec = "-"
        CreateSpeedMbs = "-"
        ExtractTimeSec = [Math]::Round($optirarNsExtractTime, 2)
        DiskExtractMbs = $optirarNsExtractSpeed
        RawDecompressMbs = $rarNsRawDecompressSpeed
        Integrity = $(if ($optirarNsVerify.Success) { "100% Pass" } else { "Fail" })
    })

    Write-Host "`n--- [BENCHMARK 4] RAR Format (Solid Stream) ---" -ForegroundColor Cyan
    $rarSolidFile = [System.IO.Path]::GetFullPath("$OutputDir\corpus_solid.rar")
    $winrarSolidOutDir = [System.IO.Path]::GetFullPath("$OutputDir\winrar_rar_solid_extracted")
    $optirarSolidOutDir = [System.IO.Path]::GetFullPath("$OutputDir\optirar_rar_solid_extracted")

    # Create Solid RAR using WinRAR (-s)
    Push-Location $srcDir
    & $winrarExe a -r -m3 -s -y -inul "$rarSolidFile" *
    Pop-Location
    $rarSolidSizeMB = [Math]::Round((Get-Item $rarSolidFile).Length / 1MB, 2)
    $rarSolidRatio = [Math]::Round(($rarSolidSizeMB / $totalGeneratedMB) * 100, 1)

    # WinRAR extract Solid RAR
    $sw.Restart()
    & $winrarExe x -y -inul $rarSolidFile "$winrarSolidOutDir\"
    $sw.Stop()
    $winrarSolidExtractTime = $sw.Elapsed.TotalSeconds
    $winrarSolidExtractSpeed = [Math]::Round($totalGeneratedMB / [Math]::Max(0.001, $winrarSolidExtractTime), 1)

    # OptiRAR extract Solid RAR
    $sw.Restart()
    & $optirarCli extract $rarSolidFile --output $optirarSolidOutDir --profile performance
    $sw.Stop()
    $optirarSolidExtractTime = $sw.Elapsed.TotalSeconds
    $optirarSolidExtractSpeed = [Math]::Round($totalGeneratedMB / [Math]::Max(0.001, $optirarSolidExtractTime), 1)
    $optirarSolidVerify = Verify-ExtractedDir $optirarSolidOutDir

    $rarSolidColor = if ($optirarSolidVerify.Success) { "Green" } else { "Red" }
    Write-Host "Solid RAR Size: $rarSolidSizeMB MB ($rarSolidRatio%)"
    Write-Host "WinRAR (Rar.exe) Extract: in $([Math]::Round($winrarSolidExtractTime, 2))s ($winrarSolidExtractSpeed MB/s)" -ForegroundColor Yellow
    Write-Host "OptiRAR Extract (Solid): in $([Math]::Round($optirarSolidExtractTime, 2))s ($optirarSolidExtractSpeed MB/s)" -ForegroundColor Green
    Write-Host "OptiRAR Solid RAR Integrity: $($optirarSolidVerify.Success) ($($optirarSolidVerify.MatchCount)/$($sourceFiles.Count) verified)" -ForegroundColor $rarSolidColor

    $results.Add([PSCustomObject]@{
        Format = "RAR (Solid)"
        Engine = "WinRAR 7.01"
        SizeMB = $rarSolidSizeMB
        RatioPct = $rarSolidRatio
        CreateTimeSec = "-"
        CreateSpeedMbs = "-"
        ExtractTimeSec = [Math]::Round($winrarSolidExtractTime, 2)
        DiskExtractMbs = $winrarSolidExtractSpeed
        RawDecompressMbs = "-"
        Integrity = "Baseline"
    })
    $results.Add([PSCustomObject]@{
        Format = "RAR (Solid)"
        Engine = "OptiRAR"
        SizeMB = $rarSolidSizeMB
        RatioPct = $rarSolidRatio
        CreateTimeSec = "-"
        CreateSpeedMbs = "-"
        ExtractTimeSec = [Math]::Round($optirarSolidExtractTime, 2)
        DiskExtractMbs = $optirarSolidExtractSpeed
        RawDecompressMbs = "-"
        Integrity = $(if ($optirarSolidVerify.Success) { "100% Pass" } else { "Fail" })
    })
}

# ====================================================================
# 4. OPTIRAR NATIVE FORMAT (.orr) BENCHMARKS
# ====================================================================
$orrLevels = @(
    @{ Name = "Fast"; CliLevel = "fast"; Description = "Zstd Level 3 (Ultra-Fast)" },
    @{ Name = "Normal"; CliLevel = "normal"; Description = "Zstd Level 9 (Balanced High-Ratio)" },
    @{ Name = "Maximum"; CliLevel = "maximum"; Description = "Zstd Level 19 (Ultra Compression)" }
)

foreach ($lvl in $orrLevels) {
    Write-Host "`n--- [BENCHMARK 5-$($lvl.Name)] Native .orr Format ($($lvl.Description)) ---" -ForegroundColor Cyan
    $orrFile = [System.IO.Path]::GetFullPath("$OutputDir\corpus_$($lvl.Name.ToLower()).orr")
    $orrOutDir = [System.IO.Path]::GetFullPath("$OutputDir\orr_$($lvl.Name.ToLower())_extracted")

    # Creation
    $sw.Restart()
    & $optirarCli create $orrFile (Get-ChildItem -Path $srcDir).FullName --level $lvl.CliLevel
    $sw.Stop()
    $orrCreateTime = $sw.Elapsed.TotalSeconds
    $orrSizeMB = [Math]::Round((Get-Item $orrFile).Length / 1MB, 2)
    $orrCreateSpeed = [Math]::Round($totalGeneratedMB / [Math]::Max(0.001, $orrCreateTime), 1)
    $orrRatio = [Math]::Round(($orrSizeMB / $totalGeneratedMB) * 100, 1)

    # Extraction to Disk
    $sw.Restart()
    & $optirarCli extract $orrFile --output $orrOutDir --profile performance
    $sw.Stop()
    $orrExtractTime = $sw.Elapsed.TotalSeconds
    $orrExtractSpeed = [Math]::Round($totalGeneratedMB / [Math]::Max(0.001, $orrExtractTime), 1)
    $orrVerify = Verify-ExtractedDir $orrOutDir

    # Raw Parallel Decompressor Speed
    $orrBenchJson = & $optirarCli benchmark $orrFile --json | ConvertFrom-Json
    $orrRawDecompressSpeed = [Math]::Round($orrBenchJson.throughput_mb_s, 1)

    $orrColor = if ($orrVerify.Success) { "Green" } else { "Red" }
    Write-Host ".orr ($($lvl.Name)) Created:   $orrSizeMB MB ($orrRatio%) in $([Math]::Round($orrCreateTime, 2))s ($orrCreateSpeed MB/s)"
    Write-Host ".orr ($($lvl.Name)) Extracted: in $([Math]::Round($orrExtractTime, 2))s ($orrExtractSpeed MB/s to disk | Raw Decompress: $orrRawDecompressSpeed MB/s)" -ForegroundColor Green
    Write-Host ".orr ($($lvl.Name)) Integrity: $($orrVerify.Success) ($($orrVerify.MatchCount)/$($sourceFiles.Count) verified)" -ForegroundColor $orrColor

    $results.Add([PSCustomObject]@{
        Format = ".orr ($($lvl.Name))"
        Engine = "OptiRAR"
        SizeMB = $orrSizeMB
        RatioPct = $orrRatio
        CreateTimeSec = [Math]::Round($orrCreateTime, 2)
        CreateSpeedMbs = $orrCreateSpeed
        ExtractTimeSec = [Math]::Round($orrExtractTime, 2)
        DiskExtractMbs = $orrExtractSpeed
        RawDecompressMbs = $orrRawDecompressSpeed
        Integrity = $(if ($orrVerify.Success) { "100% Pass" } else { "Fail" })
    })
}

# ====================================================================
# SUMMARY REPORT
# ====================================================================
Write-Host "`n=======================================================================================================" -ForegroundColor Cyan
Write-Host "                                   FINAL BENCHMARK COMPARISON TABLE                                    " -ForegroundColor Cyan
Write-Host "=======================================================================================================" -ForegroundColor Cyan
Write-Host ("{0,-18} | {1,-12} | {2,-9} | {3,-8} | {4,-12} | {5,-12} | {6,-13} | {7,-9}" -f `
    "Format", "Engine", "Size (MB)", "Ratio %", "Create(MB/s)", "Extract(MB/s)", "RawDecom(MB/s)", "Integrity")
Write-Host "-------------------------------------------------------------------------------------------------------"
foreach ($r in $results) {
    Write-Host ("{0,-18} | {1,-12} | {2,-9} | {3,-8} | {4,-12} | {5,-12} | {6,-13} | {7,-9}" -f `
        $r.Format, $r.Engine, $r.SizeMB, "$($r.RatioPct)%", $r.CreateSpeedMbs, $r.DiskExtractMbs, $r.RawDecompressMbs, $r.Integrity)
}
Write-Host "=======================================================================================================" -ForegroundColor Cyan

if (-not $KeepArtifacts) {
    Remove-Item -Path $OutputDir -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "`nBenchmark scratch directory cleaned up." -ForegroundColor Gray
}
