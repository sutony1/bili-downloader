$ErrorActionPreference = 'Stop'
$taskRoot = Join-Path $env:LOCALAPPDATA 'BiliDownloader\Transcription'
$taskEnv = Join-Path $taskRoot 'venv'
$taskPython = Join-Path $taskEnv 'Scripts\python.exe'
$taskBase = Get-Command python -ErrorAction SilentlyContinue
if (-not $taskBase) { throw '请先安装 Python 3.11 或 3.12（python.org），再运行此脚本。' }
if (-not (Test-Path -LiteralPath $taskPython)) {
    & $taskBase.Source -m venv $taskEnv
    if ($LASTEXITCODE -ne 0) { throw '无法创建语音转写环境。' }
}
& $taskPython -m pip install --index-url https://pypi.org/simple 'faster-whisper==1.2.1' 'ctranslate2==4.8.3' 'nvidia-cublas-cu12' 'nvidia-cudnn-cu12>=9,<10'
if ($LASTEXITCODE -ne 0) { throw '安装失败，请检查网络。未修改全局 Python 环境。' }
Write-Output '正在准备 large-v3 模型，约 3GB，只需下载一次。录音不会上传。'
& $taskPython -u (Join-Path $PSScriptRoot 'transcribe_audio.py') --prepare
if ($LASTEXITCODE -ne 0) { throw '模型准备失败，请检查网络后重试。' }
& $taskPython -u (Join-Path $PSScriptRoot 'transcribe_audio.py') --check
if ($LASTEXITCODE -ne 0) { throw 'GPU 或模型检查失败，请检查 NVIDIA 驱动与运行库。' }
Write-Output '准备完成。请回到“语音转文字”选项卡，打开录音并开始转写。'
