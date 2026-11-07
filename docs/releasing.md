# Windows 发版

0.7.0 使用轻量基础 ZIP：EXE、用户说明与许可材料。**不包含 Tesseract、模型权重或 ONNX Runtime DLL**。Windows 中文 OCR 为默认；PP-OCRv6 tiny 在应用设置中选择后自动下载。首次准备完成后可离线识别，截图不上传。

## 构建与打包

需要 Windows x64、PowerShell 5.1+、Windows Rust、MSVC C++ Build Tools 与 Windows SDK。Rust 无外部 crate，使用锁文件离线构建。发布者无需先安装任意识别引擎或下载模型。

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_windows.ps1
```

脚本读取 Cargo 版本，执行 release 测试与静态 CRT 编译，检查 PE 的 x64 架构及 GUI 子系统，执行编号查询自检。随后生成 `release.json`、逐文件 SHA-256，压缩、重新解压核对全部文件，并再次运行包内 EXE 查询。最后输出 ZIP 和 `.zip.sha256`。默认拒绝覆盖同版本；可加 `-Force` 或 `-OutputDirectory D:\releases`。原 `-TesseractDirectory` 参数已移除。

版本在 `windows-ocr/Cargo.toml` 修改后，用 `cargo generate-lockfile --manifest-path .\windows-ocr\Cargo.toml --offline` 同步锁文件。脚本只生成本地包，不上传或修改 Git。

```text
PokeMMO-Hint-v0.7.0-windows-x64/
  pokemmo-hint.exe
  README.txt
  Quick-Start-zh-CN.png
  release.json
  SHA256SUMS.txt
  THIRD_PARTY_NOTICES.txt
  licenses/paddle-LICENSE.txt
```

用户解压后双击 EXE，正常运行没有终端窗口。选区和引擎偏好保存在 `%LOCALAPPDATA%\PokeMMOHint\`，不随发行包分发。旧版的 `ocr/` 目录不再需要，建议将新版本解压到新文件夹。

## 首次选择引擎

齿轮 → 选择引擎 → 等待准备 → 对勾应用。取消不会改变当前引擎，已完成下载可复用。已就绪缓存再次进入设置时直接本地加载，切换下载源也不重新下载；缓存加载失败先报错，点击重试才检查并修复缓存。

- Windows OCR：检测系统简体中文 OCR，无需下载附带模型。未安装时可点击语言设置按钮，从系统的简体中文语言选项安装“光学字符识别”，然后重试。系统安装可能需要管理员权限。
- PP-OCRv6 tiny：约 4.46 MB ONNX 模型与约 17 MB 推理组件下载，解压体积另计。模型默认走 `hf-mirror.com`，可选择 HF 官方优先，失败时尝试另一源；ONNX Runtime 从 Microsoft 官方 NuGet 下载固定 1.22.0 DirectML 包，仅使用其中的 x64 原生 DLL 和 CPU 推理，不下载 DirectML 的独立 GPU 组件。CPU 路径不启用 DirectML provider。

模型核对固定 SHA-256；组件通过官方 HTTPS 源下载后记录 DLL 哈希，后续准备时复核。组件哈希是本机首次下载记录，并非仓库预置发布者签名。缓存位置为 `%LOCALAPPDATA%\PokeMMOHint\ocr\ppocrv6-tiny-9ef676d6\`。实际加载和一次空白图片推理、输出字表检查通过后才启用。若系统缺少 Microsoft VC++ x64 运行库，组件加载时会明确报错；基础 Windows OCR 不依赖这份额外运行库。

## Windows 验收

分别测试 Windows 中文 OCR、PP-OCR 的首次下载、HF 镜像与官方源切换、取消／重试、断网复用缓存、重启记住引擎。验证刷新率四档保存、旧配置恢复默认每秒 1 次、开始成功自动最小化、失败保留窗口、后台识别不触发启动忙碌光标，以及猫鼬斩、喵喵、No.573、多选区独立标签、悬停完整卡片以及最小化后的持续识别。未识别、编号冲突或引擎失败时应清除旧提示。PP 字表仍缺少 6 个物种名字的部分字，不能保证完整名字覆盖。

发版脚本验证包的构建和完整性，不将操作系统语言功能、联网下载或 OCR 准确率混作无需环境的自检。当前开发环境只能执行 Rust 测试和 Windows 适配层静态检查，无法启动 Windows PowerShell（WSL interop 不可用）；MSVC 链接、首次联网准备与真实 Windows OCR 尚需在 Windows 执行，不提供旧 EXE 冒充新版。

## Git 忽略

保留源码、Cargo.lock、事实数据、字表、PNG／ICO 与许可／验证材料。忽略 target、dist、EXE、DLL、ONNX／traineddata、下载临时文件、配置、截图、ROM 和编辑器文件。既有已跟踪产物不会因忽略规则自动取消跟踪。
