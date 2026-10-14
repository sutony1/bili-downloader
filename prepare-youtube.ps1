param([string]$Destination = $PSScriptRoot)
$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$taskRoot = [IO.Path]::GetFullPath($Destination)
New-Item -ItemType Directory -Path $taskRoot -Force | Out-Null
function Get-VerifiedFile([string]$Url, [string]$Path, [string]$Sha256) {
    if ((Test-Path -LiteralPath $Path) -and (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() -eq $Sha256) {
        Write-Output ('已有校验通过的文件：' + [IO.Path]::GetFileName($Path))
        return
    }
    $taskDownload = $Path + '.download'
    Write-Output ('正在从官方 GitHub 下载：' + [IO.Path]::GetFileName($Path))
    Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $taskDownload -TimeoutSec 300
    if ((Get-FileHash -LiteralPath $taskDownload -Algorithm SHA256).Hash.ToLowerInvariant() -ne $Sha256) { throw 'SHA256 校验不一致，未安装下载的文件。' }
    Move-Item -LiteralPath $taskDownload -Destination $Path -Force
}
Get-VerifiedFile 'https://github.com/yt-dlp/yt-dlp/releases/download/2026.08.19/yt-dlp.exe' (Join-Path $taskRoot 'yt-dlp.exe') '66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a'
$taskZip = Join-Path $taskRoot 'deno-runtime.zip'
Get-VerifiedFile 'https://github.com/denoland/deno/releases/download/v2.9.7/deno-x86_64-pc-windows-msvc.zip' $taskZip 'a0c3101b4158d1dfb7d6a78a7bf0f3de80c96bb423c152beec8beb22786f2238'
$taskExtract = Join-Path $taskRoot 'deno-runtime-extracted'
Expand-Archive -LiteralPath $taskZip -DestinationPath $taskExtract -Force
Copy-Item -LiteralPath (Join-Path $taskExtract 'deno.exe') -Destination (Join-Path $taskRoot 'deno.exe') -Force
Remove-Item -LiteralPath (Join-Path $taskExtract 'deno.exe')
Remove-Item -LiteralPath $taskExtract
Remove-Item -LiteralPath $taskZip
& (Join-Path $taskRoot 'yt-dlp.exe') --version
if ($LASTEXITCODE -ne 0) { throw 'yt-dlp 无法启动' }
& (Join-Path $taskRoot 'deno.exe') --version
if ($LASTEXITCODE -ne 0) { throw 'Deno 无法启动' }
Write-Output 'YouTube 组件准备完成，重新打开下载助手即可。FFmpeg / FFprobe 仍按 README 准备。'
