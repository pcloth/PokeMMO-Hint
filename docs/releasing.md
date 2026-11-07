# Windows 发版

目前交付内置引擎的便携 ZIP：应用 EXE、Tesseract 命令行程序、它递归依赖的运行 DLL、中文模型、启动脚本和用户说明。用户解压即可离线运行，无需管理员权限，无需安装 Tesseract，也不需要 Rust 或编译工具。

## 发布者准备

使用 64 位 Windows 10／11、Windows PowerShell 5.1 或更高版本、Windows Rust、MSVC C++ Build Tools 和 Windows SDK。编译目标为 `x86_64-pc-windows-msvc`，不使用 WSL 的 Linux Rust。源码无额外 Rust crate，Cargo 使用锁文件离线构建；缺少 MSVC 工具或 Rust 目标时需先安装。

在仓库根目录运行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_windows.ps1
```

脚本自动完成：

1. 从 `windows-ocr/Cargo.toml` 读取版本；检测输出重名。查找发布者本机的 x64 Tesseract 运行目录，或使用 `-TesseractDirectory` 指定。
2. 准备 `chi_sim.traineddata`，缺失时从官方 tessdata_fast 下载；与仓库 `release/model.sha256` 固定哈希核对。已有但不同版本的模型会报错，需先移开再重试。
3. 使用静态 MSVC CRT，执行 release 测试、编译及两条编号查询自检。发布者需准备完整的 x64 OCR 运行目录；可使用已有安装或先运行开发用 `setup.ps1 -InstallOcr -NoLaunch`。用户无需安装。
4. 校验 EXE 为 Windows x64，解析引擎与各 DLL 的 PE 普通／延迟导入表，递归收集非 Windows 系统依赖。缺少 DLL 或发现错误架构会停止打包；不复制训练工具、卸载程序和无关语言。将来源运行目录提供的许可／署名文件一同保留。
5. 生成版本／构建信息 `release.json` 和逐文件 `SHA256SUMS.txt`。
6. 压缩后重新解压，逐文件核对 SHA-256，重新遍历包内 DLL 依赖；缩减 PATH，仅用包内引擎／模型生成并识别 `No.573` 测试图片，再执行编号查询。
7. 成功后生成最终 ZIP 和外部 `.zip.sha256`，清理临时目录并恢复原 Rust 环境变量。

自定义输出目录、覆盖同版本：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_windows.ps1 -OutputDirectory D:\releases -TesseractDirectory "C:\Program Files\Tesseract-OCR" -Force
```

默认不覆盖现有包。后续版本先修改 `Cargo.toml` 中的 `version`，再执行 `cargo generate-lockfile --manifest-path .\windows-ocr\Cargo.toml --offline` 同步锁文件中的包版本，提交源码和锁文件后再发版。`-Force` 用于重新打包同版本，不会修改版本号。

## 包内结构

```text
PokeMMO-Hint-v0.5.0-windows-x64/
  pokemmo-hint.exe
  Start.cmd
  README.txt
  release.json
  SHA256SUMS.txt
  THIRD_PARTY_NOTICES.txt
  licenses/
    tessdata-fast-LICENSE.txt
    ocr-components.json
    tesseract-runtime/
      doc/
        LICENSE
        AUTHORS
  ocr/
    tesseract.exe
    libtesseract-5.dll
    ...其他运行 DLL...
    runtime.json
    tessdata/
      chi_sim.traineddata
```

不打包源码构建缓存、ROM、游戏客户端、截图或发布者的选区配置。用户屏幕选区配置在 `%LOCALAPPDATA%\PokeMMOHint\screen-regions.txt`。旧版游戏窗口坐标不能直接转换，原 `regions.txt`／`roi.txt` 保留，升级后需重新框选一次；此后屏幕选区自动保存。

用户双击 `Start.cmd`：脚本只设置本次进程的包内引擎、模型与 DLL 路径，检查包内引擎能够运行后启动应用。不调用 PowerShell、winget 或安装程序。直接启动应用 EXE 也会优先使用相邻 `ocr/` 文件夹；系统已有其他 Tesseract 不影响便携版选择。

本机已检查的 Tesseract 发行版实际依赖 **27 个运行文件（1 个 EXE、26 个 DLL）**，合计 121,408,797 字节，约 116 MiB，尚未计入中文模型。版本不同，DLL 名称和体积也可能变化，脚本按实际依赖计算。中文模型仍约 2.4 MB，包体增加主要来自引擎与 DLL。只包含这批运行文件和中文模型的临时 ZIP 样例约 39 MB（不含应用及文档），正式包体积以发版结果为准。`ocr/runtime.json` 记录版本输出、每个运行文件的 SHA-256 和系统库列表。

许可材料保留来源运行目录的 `doc/`、`licenses/`、`share/licenses/` 及顶层许可／署名文件，并提供依赖项目来源索引。DLL 各自拥有许可，模型的 Apache 许可不能替代它们；发布者应使用含完整许可材料的运行目录。脚本不将未知 DLL 默认为 Windows 系统库。

## 发布前 Windows 验收

脚本已包含单元测试和包完整性检查，仍应在实际 Windows 环境验收一次：在没有安装 Tesseract、Rust 的电脑上解压到新目录，断网且不启动 PokeMMO，双击启动；使用模拟器或图鉴页面添加、命名、拖动调整选区；开始／暂停；按钮悬停文字和弱点卡片；重启恢复配置。可用未安装 Rust 的电脑确认用户流程。

校验下载包：

```powershell
Get-FileHash .\dist\PokeMMO-Hint-v0.5.0-windows-x64.zip -Algorithm SHA256
Get-Content .\dist\PokeMMO-Hint-v0.5.0-windows-x64.zip.sha256
```

将 ZIP 和 `.sha256` 上传到对应版本的 Release，说明支持 Windows x64、内置 OCR 引擎且离线免安装、覆盖基础形态 1–649。源码是否公开与二进制发包分别决定；脚本不会自动上传、提交或推送。

## Git

`.gitignore` 已排除 `target/`、`dist/`、本地 OCR 引擎／模型、ROM、截图和配置。需要提交 `Cargo.lock`、PNG／ICO、`release/` 模板、模型固定哈希、许可证和脚本。

如果某个产物以前已经被 Git 跟踪，添加忽略规则不会自动取消跟踪；应针对实际已跟踪的产物移出索引，再保留本地文件。本次环境未连接可用的 Git 索引，没有修改任何 Git 提交或索引。

## 当前验证边界

本环境的 WSL 无法启动 Windows PowerShell（`UtilBindVsockAnyPort` 错误）。Rust 测试、Windows 适配层静态检查可在这里执行；脚本源码已检查，但真实 PowerShell 运行、MSVC 链接、包内 Windows OCR 执行和最终 ZIP 仍需在发布者 Windows 上执行。不提供以旧 EXE 冒充的新包。
