# PokeMMO 属性提示

独立 Windows OCR 辅助程序：本地识别多个选区中的中文物种名或 `No.573`／`#573`，各选区上方显示首个弱点，悬停展开完整属性卡片。支持命名、拖动调整和自动保存选区。使用 Rust 和 Tesseract，不修改游戏文件。

覆盖全国编号 1–649 的基础形态，尚不支持不同形态独立识别、特性／道具修正或实时队伍评分。Android 尚未实现。旧原生 Mod 实验文件已清理。

## 自己开发运行

Windows PowerShell，项目根目录：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\windows-ocr\setup.ps1 -InstallOcr
```

更新源码后：

```powershell
cargo run --release --manifest-path .\windows-ocr\Cargo.toml --offline
```

[完整功能、数据与验证说明](windows-ocr/README.md)

## 发版给其他人

在装好 Windows Rust 和 MSVC 构建工具的电脑上，退出旧程序并运行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_windows.ps1
```

脚本读取 `windows-ocr/Cargo.toml` 的版本号，测试、编译并打包到 `dist/`：

- `PokeMMO-Hint-v0.4.1-windows-x64.zip`
- `PokeMMO-Hint-v0.4.1-windows-x64.zip.sha256`

将这两个文件上传到 GitHub/Gitee Release 或直接发送即可。用户解压整个 ZIP，双击 `Start.cmd`，首次需要联网安装 Tesseract 引擎；包内已带中文模型，不需要 Rust、编译器或 ROM。

后续修改 `Cargo.toml` 的 `version`，先执行 `cargo generate-lockfile --manifest-path .\windows-ocr\Cargo.toml --offline` 同步锁文件，再运行脚本。同版本重新打包可加 `-Force`。脚本只生成本地包，不自动上传或推送 Git。

[打包参数、交付清单与验收方法](docs/releasing.md)

## 仓库内容

- `windows-ocr/`：程序、图标、数据和安装／启动模板。
- `scripts/`：Windows 发版与本地数据导入、核对工具。
- `docs/`：验证记录与事实核对证据。
- `.gitignore`：排除构建缓存、发行包、OCR 引擎／模型、ROM、本地配置与截图；保留锁文件和图标。

源码检查和本机五 ROM 数据核对已通过。当前开发环境不能启动 Windows 程序，PowerShell 发版、实际 UI 和干净机器安装需要在 Windows 上验收；没有将旧 EXE 重新命名为新版发行包。
