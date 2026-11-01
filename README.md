# 视频下载助手 v0.4.0：B站 / 抖音 / YouTube / 语音转文字

新增“语音转文字”选项卡：打开手机 M4A、MP3、WAV 或视频文件，使用本机 NVIDIA GPU 与 Faster-Whisper large-v3 转写为 UTF-8 TXT。模型使用 INT8/FP16 计算，不会自动改用 CPU，也不会上传录音。界面支持中文、英语或自动识别，显示转写进度，可停止任务及打开结果。

TXT 保存在录音文件旁，文件名为“原文件名_转写.txt”，重名时自动编号，不覆盖录音或已有文本。文本为自动识别结果，未做说话人分离；人名、地名、数字仍需核对。

首次使用：需要 NVIDIA GPU、兼容 CUDA 12 的驱动、Python 3.11/3.12 和 FFmpeg。点击选项卡里的“首次使用：准备转写环境”，或运行同目录“准备语音转写环境.cmd”。脚本建立独立 Python 环境，安装 Faster-Whisper/CUDA 运行库，并从 Systran/faster-whisper-large-v3 下载约3GB模型。准备时需要联网，正式转写只使用本机文件与缓存。已有本机 Faster-Whisper 环境和模型时可直接点“检查环境”。显存不足时先关闭其他 GPU 应用，不会悄悄换成 small 模型或 CPU。

模型查找顺序：BILI_WHISPER_MODEL 环境变量、程序目录 models/faster-whisper-large-v3、用户 Models/whisper/faster-whisper-large-v3、Hugging Face 本地缓存。可用 BILI_WHISPER_PYTHON 指定 Python；默认优先使用准备脚本创建的独立环境，其次使用本机 Python。

三种平台的下载与登录方式如下：

# 视频下载助手：B站 / 抖音 / YouTube

用 Rust 编写的 Windows 桌面视频下载工具，当前版本为 **v0.4.0 试用版**。提供“B站下载”“抖音下载”“YouTube 下载”和“语音转文字”四个选项卡；三个下载平台分别保存网址、解析结果和本机网页登录状态。B站继续下载音视频并合并成 MP4；抖音和 YouTube 支持单条视频、可用画质选择和文件核验。

新增抖音功能前的稳定版已保存为 [pre-douyin-v0.1.6 回退标签](https://github.com/sutony1/bili-downloader/tree/pre-douyin-v0.1.6)。仍需原版时，可下载 [v0.1.6 稳定版](https://github.com/sutony1/bili-downloader/releases/tag/v0.1.6)。

## 在另一台电脑直接运行

1. 从 [v0.4.0 发布页](https://github.com/sutony1/bili-downloader/releases/tag/v0.4.0) 下载 Windows x64 ZIP 并完整解压。
2. 按下面的“准备 WebView2”步骤安装或确认已有浏览器运行组件。
3. 按下面的“准备 FFmpeg 和 FFprobe”步骤，将两个 EXE 放在下载助手旁边。
4. 双击 `bili-downloader.exe`，选择对应平台的选项卡。需要登录时点击“登录 / 显示浏览器”，在对应的官方网页操作。B站支持扫码或 QQ 登录；抖音按网页提示扫码或手机号登录。**各平台登录独立，换电脑需要首次登录。YouTube 公开视频通常无需登录；需要账号时才勾选使用登录状态。**
5. B站：粘贴视频链接或 BV 编号 →“解析视频”→“下载最高画质”。抖音 / YouTube：粘贴单条视频链接 →“解析视频”→选择画质→“下载所选画质”。完成后用“打开下载文件夹”查看文件；“清空网址”会清空网址和旧解析结果。

运行成品不需要安装 Rust、Node.js 或 Visual Studio。程序调用 Windows 的 `curl.exe` 下载媒体，需要该命令可用。

视频保存到当前 Windows 用户的 `Downloads` 文件夹。内置网页始终静音，下载完成或失败后暂停网页音视频；保存的视频保留原有声音。已有同名文件会使用新的编号，避免覆盖。

### YouTube 组件与使用方法

新增 YouTube 功能前的 v0.2.0 已保存为 [pre-youtube-v0.2.0 回退标签](https://github.com/sutony1/bili-downloader/tree/pre-youtube-v0.2.0)，可继续下载 [v0.2.0 版本](https://github.com/sutony1/bili-downloader/releases/tag/v0.2.0)。

1. 解压程序后，双击同目录的 **准备YouTube组件.cmd**。脚本 `prepare-youtube.ps1` 会从官方 GitHub 下载 yt-dlp 2026.08.19 和 Deno 2.9.7，核对 SHA256 后将 `yt-dlp.exe`、`deno.exe` 放到程序旁边，不需管理员权限，也不修改环境变量。组件约数十 MB，需能够访问 GitHub。
2. 如果脚本下载失败，可从 [yt-dlp 官方发布页](https://github.com/yt-dlp/yt-dlp/releases) 下载 **yt-dlp.exe**；从 [Deno 官方发布页](https://github.com/denoland/deno/releases) 下载 **deno-x86_64-pc-windows-msvc.zip**，解压出 **deno.exe**，两者放在 `bili-downloader.exe` 旁边。已有 Node.js 22+ 时程序也能调用 Node；FFmpeg / FFprobe 仍按下文准备。
3. 在“YouTube 下载”选项卡粘贴视频链接，点击“解析视频”，选择画质后下载。支持 `youtube.com/watch?v=...`、`youtu.be/...`、Shorts；带播放列表参数的单条视频只下载该视频。暂不支持整个播放列表、正在直播或未开播的视频。
4. 画质按分辨率、帧率排序，同等画质优先 H.264；下拉框标明编码及容器。MP4 版本直接保存或与原音轨合并，WebM 视频轨保存为 MKV，均不重新压缩。FFprobe 检查所选分辨率、完整时长与音轨。
5. 公开视频优先不登录直接解析；需要账号时点击“登录 / 显示浏览器”，在官方网页登录后勾选 **使用本程序中保存的 YouTube 登录状态**，再解析。Google 可能限制内置浏览器登录，受限视频、地区权限、验证及 PO Token 场景尚未全面覆盖；不要把公开视频测试通过理解为所有账号视频都能下载。

YouTube 登录保存在 `%LOCALAPPDATA%\BiliDownloader\YouTubeBrowserProfile`，与其他平台、系统 Edge 分开。勾选使用登录状态时，只把本程序中 YouTube 域的 Cookie 临时交给本机 yt-dlp，任务结束自动删除任务文件；不会读取其他平台账号，也不需要把密码或 Cookie 粘贴到界面。

电脑网络必须能访问 YouTube。yt-dlp 使用已有网络代理配置，组件准备脚本使用 Windows 网络设置，程序不修改系统代理。若浏览器可播放但程序请求失败，请检查代理是否覆盖桌面程序、组件版本以及网站返回的错误。

YouTube 的 JavaScript 挑战使用 yt-dlp 官方解析组件及 Deno/Node 运行时，参见 [官方 EJS 说明](https://github.com/yt-dlp/yt-dlp/wiki/EJS)；官方 Windows yt-dlp.exe 已包含相应 EJS 脚本。网站机制变化时可按官方说明更新组件。安装脚本中的版本和 SHA256 为本次验证版本。

### 抖音试用范围与画质说明

- 支持 `https://www.douyin.com/video/作品编号`、分享短链接 `https://v.douyin.com/.../`，以及带明确 `modal_id` 的作品链接；会转换为对应的单条视频页面。
- 不支持仅含关键词的搜索页、搜索列表、图文、直播或批量下载。
- 网页返回多个视频版本时，画质下拉框显示分辨率和可取得的码率，默认按分辨率、码率选择最高可用版本。
- 若仅能读取播放器正在使用的地址，下拉框会标注“当前播放画质”，并提示**不代表作品全部版本**。可显示浏览器，登录并调整播放器画质后重新解析；不会把低清视频放大并声称高清。
- 需要登录或验证时在官方网页完成，再重新解析；程序不自行生成或破解接口签名。
- 普通短视频直接保存已有音视频文件，不重新压缩；FFprobe 检查视频流、分辨率和完整时长。源视频没有音轨时，会明确提示。
- 抖音网页变化、账号权限、资源过期或特殊封装可能导致解析/下载失败。这是试用版，尚未验证所有作品类型与登录验证场景。

网页登录资料分别保存在 `%LOCALAPPDATA%\BiliDownloader\BrowserProfile`（B站，沿用原版）和 `%LOCALAPPDATA%\BiliDownloader\DouyinBrowserProfile`（抖音）。两者也与系统 Edge 的登录独立；程序不要求手工粘贴密码或 Cookie。

### 准备 WebView2

WebView2 是程序显示 B站网页和登录窗口所用的运行组件。需要安装的是 **WebView2 Runtime**。

1. 打开 [微软 WebView2 下载页面](https://developer.microsoft.com/en-us/microsoft-edge/webview2/#download-section)，找到 **Download the WebView2 Runtime** 区域。
2. 按电脑联网情况选择安装方式：

   | 情况 | 下载选项 | 操作 |
   | --- | --- | --- |
   | 学校电脑可以联网下载安装 | **Evergreen Bootstrapper** | 点击 Download，保存并双击安装程序；它会自动下载适合当前电脑的运行组件。 |
   | 想先在家下载，再用U盘带到学校 | **Evergreen Standalone Installer → x64** | 下载完整安装包，复制到学校的 Windows x64 电脑后双击安装。安装组件这一步不需要联网。 |

3. 按提示完成安装，然后重新打开下载助手。如果安装程序提示已经安装了运行组件，可直接继续使用。
4. 点击下载助手中的“登录 / 显示浏览器”。能够打开 B站网页，就可以继续登录。

离线安装包的文件名通常为 `MicrosoftEdgeWebView2RuntimeInstallerX64.exe`；它用于安装运行组件，不需要放进下载助手文件夹。若学校电脑限制安装软件，请让管理员协助安装。上述两种安装方式见 [微软的运行组件部署说明](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution)。

### 准备 FFmpeg 和 FFprobe

这两个工具从**同一个压缩包**中获取：`ffmpeg.exe` 用于合并音视频，`ffprobe.exe` 用于检查分辨率、时长和音视频流。

推荐使用“放在程序旁边”的方式，操作如下：

1. 打开 [FFmpeg 官方下载页面](https://ffmpeg.org/download.html)，在 **Windows EXE Files** 下选择 **Windows builds from gyan.dev**。也可以直接打开 [Gyan 的 Windows 构建页面](https://www.gyan.dev/ffmpeg/builds/)。
2. 找到 **release builds** 区域，点击 **ffmpeg-release-essentials.zip**。这个 ZIP 包包含本程序需要的工具，Windows 可以直接解压。
3. 右键下载的 ZIP，选择“全部提取”。打开解压出来的 FFmpeg 文件夹，再进入其中的 **bin** 文件夹。
4. 从 `bin` 中复制 **ffmpeg.exe、ffprobe.exe**，粘贴到 `bili-downloader.exe` 所在的文件夹。两者需要与下载助手处于同一层目录，不能只把整个 FFmpeg 文件夹放在旁边。

准备完成后，文件夹应类似这样：

```text
B站下载助手/
├─ bili-downloader.exe
├─ ffmpeg.exe
├─ ffprobe.exe
├─ README.md
└─ 使用说明.txt
```

**这种方式不需要配置环境变量。** 如果在家提前准备，可以把整个文件夹复制到U盘，再拷到学校电脑的可写文件夹中运行；学校电脑仍需要 WebView2 Runtime，以及访问 B站的网络。

### 确认准备成功

在上面这个文件夹的空白处，按住 Shift 后点击鼠标右键，选择“在此处打开 PowerShell 窗口”或“在终端中打开”，依次执行：

```powershell
.\ffmpeg.exe -version
.\ffprobe.exe -version
curl.exe --version
```

前两个命令应分别显示以 `ffmpeg version`、`ffprobe version` 开头的版本信息；第三个命令应显示 curl 的版本。版本号可能随下载时间变化。

若提示找不到 `ffmpeg.exe` 或 `ffprobe.exe`，检查两个文件是否确实放在当前文件夹中。直接双击这些命令行工具时窗口可能瞬间关闭，用上述命令检查更直观。

最后双击 `bili-downloader.exe`，打开内置浏览器并登录 B站，然后试着解析、下载一个可访问的单P视频。更换电脑需要在本程序中首次登录；登录状态会保存在该电脑上。

## 从源码复现

支持环境：**Windows 10/11 x64，MSVC 工具链**。当前实现使用 Windows 专用 API，未实现 Linux/macOS 版本。

准备：

- 安装 [Rust/rustup](https://rust-lang.org/tools/install/) 的 Windows x64 版本，按提示准备 Visual Studio C++ Build Tools 和 Windows SDK。
- 本项目通过 `rust-toolchain.toml` 固定 Rust 1.88.0，并提交 `Cargo.lock` 固定依赖版本。
- 安装 WebView2 Runtime。实际下载另需 FFmpeg、FFprobe 和 `curl.exe`。
- 如需运行 JavaScript 测试，安装 Node.js 22 或更高版本。

在 PowerShell 中运行：

```powershell
git clone https://github.com/sutony1/bili-downloader.git
cd bili-downloader
git checkout codex/progress-stop-v0.3.2
cargo build --release --locked
.\target\release\bili-downloader.exe
```

第一次构建需要联网下载 Rust 工具链和 Cargo 依赖。如出现 `link.exe` 未找到，检查 MSVC C++ Build Tools 安装是否完整。

构建并打包可使用：

```powershell
powershell -ExecutionPolicy Bypass -File .\build.ps1
```

输出 ZIP 位于 `dist`。打包脚本只包含程序、README 和使用说明，不包含 FFmpeg、yt-dlp/Deno 二进制、浏览器配置或测试视频。

## 验证与排错

不需要登录的源码测试：

```powershell
cargo test --locked
node tests/extract.test.cjs
node tests/douyin.test.cjs
```

在本机验证界面和播放控制：

```powershell
.\target\release\bili-downloader.exe --smoke-test --isolated-test-profile
.\target\release\bili-downloader.exe --media-smoke-test --isolated-test-profile
```

程序退出后，检查 EXE 同目录中的 `smoke-test.txt` 和 `media-smoke-test.txt` 是否以 `PASS` 开头。媒体测试检查真实播放的测试音频在下载结束事件后被暂停，并检查 WebView2 静音状态。

登录后可用你要下载的视频编号检查真实解析：

```powershell
.\target\release\bili-downloader.exe --parse-smoke-test <BV编号>
```

将 `<BV编号>` 替换为实际编号，不包含尖括号。结果写入 EXE 同目录的 `parse-smoke-test.json`。这个测试需要当前电脑已在本程序中登录、网络能访问 B站，以及视频可访问。可附加 `--delayed-navigation` 验证延迟加载。

抖音真实解析可使用以下命令，将 `<单条视频链接>` 替换为自己的链接：

```powershell
.\target\release\bili-downloader.exe --douyin-smoke-test "<单条视频链接>"
```

结果写入 EXE 同目录的 `douyin-test.json`，不会写入临时播放地址或 Cookie。附加 `--download-test` 可进一步下载并核验，会在 Windows 下载文件夹中保存视频。网页登录和带登录凭据的 CDN 请求仍需人工在官方网页完成登录；本次自动测试不覆盖所有登录验证方式。

常见情况：

- **未找到 FFmpeg**：同时准备 `ffmpeg.exe` 和 `ffprobe.exe`，放在程序旁边。
- **解析失败/超时**：点击“登录 / 显示浏览器”，确认登录和目标页面能正常加载后重试。B站最多等待 35 秒，抖音最多等待 50 秒，YouTube 解析最多等待 110 秒。
- **网页跳转异常**：`login-diagnostic.txt` 记录跳转域名与路径，不记录查询参数、Cookie 或账号信息。
- **下载链接过期**：重新解析后再下载。下载失败时暂存文件保留在下载文件夹的 `.bili-` 子目录；成功后自动清理。

## 实现与范围

界面使用 Tao + Wry/WebView2，Rust 负责任务调度、下载和核验，JavaScript 在登录后的 B站页面中调用视频接口。优先选取接口返回的最高画质，接口不可用时尝试当前播放器资源。画质取决于视频源、账号权限和 B站响应，不能保证所有视频都有 1080P/4K。

音视频分别下载，FFmpeg 以 `-c copy` 合并，FFprobe 核验音视频流和时长。支持 B站 `bilivideo.com`、`bilivideo.cn` 资源地址，并优先选取常规 CDN 备用地址。

当前支持 B站单P视频、抖音普通单条短视频、YouTube 单条视频和单个下载任务。下载时可以点击“停止下载”，终止当前任务及它启动的下载、合并进程。按钮显示“正在停止…”时等待进程退出；随后可以重新选择画质、重新下载或清空网址。未完成的临时文件保留在下载文件夹。本版尚无暂停/继续按钮、批量队列或安装包。网站接口和验证机制变化可能影响解析。解析或下载期间暂时锁定选项卡，避免任务与当前界面错位；各平台的结果分别保存。

登录状态保存在本机 `%LOCALAPPDATA%\BiliDownloader\BrowserProfile`，与 Edge/Codex 浏览器分别保存。程序通过 B站及 QQ 官方网页完成登录；仓库和发行包不包含浏览器配置、Cookie、测试下载内容或临时播放链接。

## 版本记录

- v0.4.0（试用）：新增本地 NVIDIA GPU 语音转文字选项卡，使用 Faster-Whisper large-v3 和 INT8/FP16；支持录音或视频、语言选择、进度与停止、TXT 保存及打开，附独立环境和模型准备脚本。

- v0.3.2（试用）：YouTube 进度改用数字字段，在程序内生成中文百分比、文件大小、速度和预计剩余时间；三个平台增加“停止下载”，终止当前下载进程树后恢复画质选择和网址输入。

- v0.3.1（试用）：抖音、YouTube 的登录状态检测及下载前 Cookie 读取改用 WebView2 异步回调；界面线程不再同步等待或运行嵌套消息循环。读取超时会显示错误；关闭登录浏览器后停止定期登录检测。针对解析或下载后拖动窗口卡死的报告作此修复，仍需用户验证原先的复现步骤。

- v0.3.0（试用）：新增 YouTube 选项卡、独立登录目录、可用画质选择、音视频下载合并与核验，附官方组件准备脚本。

- v0.2.0（试用）：两个平台选项卡，独立抖音网页登录目录，单条作品链接解析、可用画质选择与 MP4 核验；保留 B站功能和原有登录目录。
- v0.1.6：新增“清空网址”，清空后自动聚焦输入框。
- v0.1.5：修复 `bilivideo.cn` 音频解析，增加页面等待、超时和错误反馈。
- v0.1.4：网页静音，下载结束后暂停网页音视频。
- v0.1.3：登录状态检测、授权后恢复 B站页面。
- v0.1.2：QQ 授权弹窗和回跳支持。
- v0.1.1：修复本地界面加载白屏。

请仅下载你有权保存的内容，程序使用账号已有访问权限，不绕过付费或验证限制。
