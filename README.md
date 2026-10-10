# B站下载助手

用 Rust 编写的 Windows 桌面视频下载工具，当前版本 **v0.1.6**。通过内置 B站网页登录，获取当前账号可以访问的画质，下载音视频并合并成 MP4。

## 在另一台电脑直接运行

1. 在 [Releases](https://github.com/sutony1/bili-downloader/releases/latest) 下载 Windows x64 ZIP，解压到可写文件夹。
2. 按下面的“准备 WebView2”步骤安装或确认已有浏览器运行组件。
3. 按下面的“准备 FFmpeg 和 FFprobe”步骤，将两个 EXE 放在下载助手旁边。
4. 双击 `bili-downloader.exe`，点击“登录 / 显示浏览器”，在 B站网页扫码或使用 QQ 登录。**换到另一台电脑需要首次登录。**
5. 粘贴视频链接或 BV 编号 →“解析视频”→“下载最高画质”。完成后用“打开下载文件夹”查看文件；“清空网址”方便继续粘贴下一个链接。

运行成品不需要安装 Rust、Node.js 或 Visual Studio。程序调用 Windows 的 `curl.exe` 下载媒体，需要该命令可用。

视频保存到当前 Windows 用户的 `Downloads` 文件夹。内置网页始终静音，下载完成或失败后暂停网页音视频；保存的视频保留原有声音。已有同名文件会使用新的编号，避免覆盖。

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
cargo build --release --locked
.\target\release\bili-downloader.exe
```

第一次构建需要联网下载 Rust 工具链和 Cargo 依赖。如出现 `link.exe` 未找到，检查 MSVC C++ Build Tools 安装是否完整。

构建并打包可使用：

```powershell
powershell -ExecutionPolicy Bypass -File .\build.ps1
```

输出 ZIP 位于 `dist`。打包脚本只包含程序、README 和使用说明，不包含 FFmpeg、浏览器配置或测试视频。

## 验证与排错

不需要登录的源码测试：

```powershell
cargo test --locked
node tests/extract.test.cjs
```

在本机验证界面和播放控制：

```powershell
.\target\release\bili-downloader.exe --smoke-test
.\target\release\bili-downloader.exe --media-smoke-test
```

程序退出后，检查 EXE 同目录中的 `smoke-test.txt` 和 `media-smoke-test.txt` 是否以 `PASS` 开头。媒体测试检查真实播放的测试音频在下载结束事件后被暂停，并检查 WebView2 静音状态。

登录后可用你要下载的视频编号检查真实解析：

```powershell
.\target\release\bili-downloader.exe --parse-smoke-test <BV编号>
```

将 `<BV编号>` 替换为实际编号，不包含尖括号。结果写入 EXE 同目录的 `parse-smoke-test.json`。这个测试需要当前电脑已在本程序中登录、网络能访问 B站，以及视频可访问。可附加 `--delayed-navigation` 验证延迟加载。

常见情况：

- **未找到 FFmpeg**：同时准备 `ffmpeg.exe` 和 `ffprobe.exe`，放在程序旁边。
- **解析失败/超时**：点击“登录 / 显示浏览器”，确认登录和目标页面能正常加载后重试。解析整体最多等待 35 秒，接口请求有超时和重试。
- **网页跳转异常**：`login-diagnostic.txt` 记录跳转域名与路径，不记录查询参数、Cookie 或账号信息。
- **下载链接过期**：重新解析后再下载。下载失败时暂存文件保留在下载文件夹的 `.bili-` 子目录；成功后自动清理。

## 实现与范围

界面使用 Tao + Wry/WebView2，Rust 负责任务调度、下载和核验，JavaScript 在登录后的 B站页面中调用视频接口。优先选取接口返回的最高画质，接口不可用时尝试当前播放器资源。画质取决于视频源、账号权限和 B站响应，不能保证所有视频都有 1080P/4K。

音视频分别下载，FFmpeg 以 `-c copy` 合并，FFprobe 核验音视频流和时长。支持 B站 `bilivideo.com`、`bilivideo.cn` 资源地址，并优先选取常规 CDN 备用地址。

当前仅支持单P视频和单个下载任务，尚无批量队列、取消、断点续传或安装包。网站接口和验证机制变化可能影响解析。

登录状态保存在本机 `%LOCALAPPDATA%\BiliDownloader\BrowserProfile`，与 Edge/Codex 浏览器分别保存。程序通过 B站及 QQ 官方网页完成登录；仓库和发行包不包含浏览器配置、Cookie、测试下载内容或临时播放链接。

## 版本记录

- v0.1.6：新增“清空网址”，清空后自动聚焦输入框。
- v0.1.5：修复 `bilivideo.cn` 音频解析，增加页面等待、超时和错误反馈。
- v0.1.4：网页静音，下载结束后暂停网页音视频。
- v0.1.3：登录状态检测、授权后恢复 B站页面。
- v0.1.2：QQ 授权弹窗和回跳支持。
- v0.1.1：修复本地界面加载白屏。

请仅下载你有权保存的内容，程序使用账号已有访问权限，不绕过付费或验证限制。
