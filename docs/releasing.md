# Windows 发版

目前交付轻量 ZIP：应用 EXE、中文模型、启动／引擎安装脚本、用户说明和第三方许可证。Tesseract 引擎通过现有安装或首次 winget 安装使用，避免直接复制本机大量 DLL。首次安装需要网络，之后 OCR 本地离线运行。

## 发布者准备

使用 64 位 Windows 10／11、Windows PowerShell 5.1 或更高版本、Windows Rust、MSVC C++ Build Tools 和 Windows SDK。编译目标为 `x86_64-pc-windows-msvc`，不使用 WSL 的 Linux Rust。源码无额外 Rust crate，Cargo 使用锁文件离线构建；缺少 MSVC 工具或 Rust 目标时需先安装。

在仓库根目录运行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_windows.ps1
```

脚本自动完成：

1. 从 `windows-ocr/Cargo.toml` 读取版本；检测输出重名。
2. 准备 `chi_sim.traineddata`，缺失时从官方 tessdata_fast 下载；与仓库 `release/model.sha256` 固定哈希核对。已有但不同版本的模型会报错，需先移开再重试。
3. 使用静态 MSVC CRT，执行 release 测试、编译及两条编号查询自检。打包不要求发布者本机安装 OCR 引擎。
4. 校验 EXE 为 Windows x64，将明确列出的文件放入临时目录。
5. 生成版本／构建信息 `release.json` 和逐文件 `SHA256SUMS.txt`。
6. 压缩后重新解压，逐文件核对 SHA-256，并再次执行 EXE 编号查询。
7. 成功后生成最终 ZIP 和外部 `.zip.sha256`，清理临时目录并恢复原 Rust 环境变量。

自定义输出目录、覆盖同版本：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_windows.ps1 -OutputDirectory D:\releases -Force
```

默认不覆盖现有包。后续版本先修改 `Cargo.toml` 中的 `version`，再执行 `cargo generate-lockfile --manifest-path .\windows-ocr\Cargo.toml --offline` 同步锁文件中的包版本，提交源码和锁文件后再发版。`-Force` 用于重新打包同版本，不会修改版本号。

## 包内结构

```text
PokeMMO-Hint-v0.4.1-windows-x64/
  pokemmo-hint.exe
  Start.cmd
  install-ocr.ps1
  README.txt
  release.json
  SHA256SUMS.txt
  THIRD_PARTY_NOTICES.txt
  licenses/
    tessdata-fast-LICENSE.txt
  ocr/
    tessdata/
      chi_sim.traineddata
```

不打包源码构建缓存、ROM、游戏客户端、截图或发布者的选区配置。用户配置仍在 `%LOCALAPPDATA%\PokeMMOHint\regions.txt`，升级时保留。

用户双击 `Start.cmd`：启动脚本查找已安装的 Tesseract，缺失时调用 winget 安装 `UB-Mannheim.TesseractOCR`，可能出现 Windows 安装确认。找到后校验引擎与中文模型，再将这两个路径明确传给应用。用户自定义位置可通过 `POKEMMO_TESSERACT` 指定。没有 winget 时，按包内 README 手动安装引擎。

## 发布前 Windows 验收

脚本已包含单元测试和包完整性检查，仍应在实际 Windows 环境验收一次：解压到新目录，双击启动；第一次安装 OCR；添加、命名、拖动调整选区；开始／暂停；按钮悬停文字和弱点卡片；重启恢复配置。可用未安装 Rust 的电脑确认用户流程。

校验下载包：

```powershell
Get-FileHash .\dist\PokeMMO-Hint-v0.4.1-windows-x64.zip -Algorithm SHA256
Get-Content .\dist\PokeMMO-Hint-v0.4.1-windows-x64.zip.sha256
```

将 ZIP 和 `.sha256` 上传到对应版本的 Release，说明支持 Windows x64、首次需安装 OCR、覆盖基础形态 1–649。源码是否公开与二进制发包分别决定；脚本不会自动上传、提交或推送。

## Git

`.gitignore` 已排除 `target/`、`dist/`、本地 OCR 引擎／模型、ROM、截图和配置。需要提交 `Cargo.lock`、PNG／ICO、`release/` 模板、模型固定哈希、许可证和脚本。

如果某个产物以前已经被 Git 跟踪，添加忽略规则不会自动取消跟踪；应针对实际已跟踪的产物移出索引，再保留本地文件。本次环境未连接可用的 Git 索引，没有修改任何 Git 提交或索引。

## 当前验证边界

本环境的 WSL 无法启动 Windows PowerShell（`UtilBindVsockAnyPort` 错误）。Rust 测试、Windows 适配层静态检查可在这里执行；脚本源码已检查，但真实 PowerShell 运行、MSVC 链接、安装器和最终 ZIP 仍需在发布者 Windows 上执行。不提供以旧 EXE 冒充的新包。
