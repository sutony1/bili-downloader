param([switch]$Test)
$ErrorActionPreference = 'Stop'
Push-Location -LiteralPath $PSScriptRoot
try {
    if ($Test) {
        cargo test --locked
        if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
        node tests/extract.test.cjs
        if ($LASTEXITCODE -ne 0) { throw 'JavaScript tests failed.' }
        node tests/douyin.test.cjs
        if ($LASTEXITCODE -ne 0) { throw 'Douyin JavaScript tests failed.' }
    }
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    $taskManifest = Get-Content -LiteralPath 'Cargo.toml' -Raw
    $taskMatch = [regex]::Match($taskManifest, '(?m)^version\s*=\s*"([^"]+)"')
    if (-not $taskMatch.Success) { throw 'Package version not found.' }
    $taskVersion = $taskMatch.Groups[1].Value
    $taskName = "bili-downloader-v$taskVersion-windows-x64"
    $taskPackage = Join-Path $PSScriptRoot "dist\$taskName"
    New-Item -ItemType Directory -Path $taskPackage -Force | Out-Null
    Copy-Item -LiteralPath 'target\release\bili-downloader.exe','README.md','使用说明.txt','准备YouTube组件.cmd','prepare-youtube.ps1','transcribe_audio.py','prepare-transcription.ps1','准备语音转写环境.cmd' -Destination $taskPackage -Force
    $taskFiles = @('bili-downloader.exe','README.md','使用说明.txt','准备YouTube组件.cmd','prepare-youtube.ps1','transcribe_audio.py','prepare-transcription.ps1','准备语音转写环境.cmd') | ForEach-Object { Join-Path $taskPackage $_ }
    $taskArchive = Join-Path $PSScriptRoot "dist\$taskName.zip"
    Compress-Archive -LiteralPath $taskFiles -DestinationPath $taskArchive -Force
    $taskHash = (Get-FileHash -LiteralPath $taskArchive -Algorithm SHA256).Hash.ToLowerInvariant()
    "$taskHash  $taskName.zip" | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'dist\SHA256SUMS.txt') -Encoding ASCII
    Write-Output "Package: $taskArchive"
} finally {
    Pop-Location
}
