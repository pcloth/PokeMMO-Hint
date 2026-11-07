# Windows OCR 属性提示（Rust 0.5.0）

这是独立 Windows 程序，不是 `.mod` 包。直接截取用户框选的屏幕区域，无需检测 PokeMMO 进程；可用于模拟器、其他宝可梦游戏或图鉴页面，物种及相克规则需在现有数据范围内。Rust 原生窗口、截图和属性匹配不依赖第三方 Rust crate；OCR 使用 Tesseract 5 和简体中文模型。应用运行时本地识别，不发送截图或查询到云端。

0.5.0 支持多个选区持续识别，每个成功匹配的选区上方显示独立的弱点小标签，鼠标悬停展开完整信息卡。选区和主窗口位置自动保存。用户已确认前版卡片界面改善；新版已通过核心测试和 Windows 适配层静态检查，本环境不能启动 Windows EXE，**多选区与浮动标签需要本机重新编译验证**。

## 更新已有版本

先退出旧程序，再在项目目录运行：

```powershell
cargo run --release --manifest-path .\windows-ocr\Cargo.toml --offline
```

无需重新安装引擎或下载已准备好的模型。之后可以继续使用 `run.cmd` 启动。

0.4.1 将按钮提示改为应用自行绘制的置顶文字浮窗，移开鼠标、切换应用或最小化后隐藏；开始／暂停和候选确认会显示当前文字。提示不依赖 Windows 公共控件版本，不拦截鼠标操作。

完整结果继续采用编号／名字、属性、弱点、抗性、免疫的分区卡片。标题栏显示“PokeMMO 属性提示”。应用按钮均采用蓝金色生成图标，鼠标悬停约 0.3 秒显示当前按钮的文字提示，键盘聚焦仍保留原功能名称。第一排从左到右为开始／暂停、添加选区、删除当前选区、清空选区。

选区下拉框可以直接编辑名称，点击旁边软盘图标“保存名称”生效并自动保存；最多 32 个字，留空恢复默认名称。重命名不会中断识别。

保存图标左侧的“四向箭头选框”用于查看和调整当前选区：当前选区高亮，其他适用选区显示为灰色参考框。拖动框内移动位置，拖动边缘／角调整大小；Enter 保存，Esc 取消。修改期间暂停 OCR，关闭编辑器后恢复此前识别状态，名称不变。拖动限制在当前虚拟桌面范围内，最小 8×8 像素；支持副屏的负坐标。显示器布局或屏幕分辨率改变后，需要重新添加适用选区。

标准标题栏的最小化仍可使用，最小化不暂停识别。

## 源码开发首次启动

在 Windows PowerShell 中运行：

```powershell
cd D:\codes\tools\pokemmo-mod
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\windows-ocr\setup.ps1 -InstallOcr
```

脚本检查 Windows Cargo，通过 winget 安装缺少的 Tesseract（可能弹出系统安装确认），准备中文模型，运行测试、编译并启动窗口。如果已装 Tesseract，可省略 `-InstallOcr`。下载需要网络，运行识别不需要网络。安装器来自 [Tesseract 文档列出的 UB Mannheim Windows 发行版](https://tesseract-ocr.github.io/tessdoc/Installation.html)，脚本使用其 winget 包 `UB-Mannheim.TesseractOCR`；不是 PokeMMO 官方程序。

中文模型优先复用本机已安装版本，否则从 [tessdata_fast 官方仓库](https://github.com/tesseract-ocr/tessdata_fast)下载 `chi_sim.traineddata`，约 2.35 MB。**这只是模型大小，不是安装器或整个程序占用。** 首次模型就绪后保存到本项目 `ocr/tessdata/`。

如果报 `link.exe` 或 Windows SDK 缺失，需要 Visual Studio 的“使用 C++ 的桌面开发”构建工具；仅安装 Rust 不保证 MSVC 链接环境齐全。脚本不会自动安装 Visual Studio。

之后双击 `windows-ocr/run.cmd` 即可启动。输出程序为 `windows-ocr/target/release/pokemmo-hint.exe`。

## 使用

1. 打开任意游戏、模拟器或图鉴页面，让中文物种名／编号在屏幕上可见。无需启动 PokeMMO。如果还在使用旧物种名修改 Mod，应先禁用以避免文字污染。
2. 点击“添加选区”，拖框选择一只敌人的名字那一行；可以包含等级和性别，尽量排除血条。Esc 取消。可在不同战斗布局分别添加，最多保存 16 个选区，每个选区只框一只宝可梦。
3. 可以继续添加群怪各名字的位置、单人／玩家对战位置，或图鉴的 `No.573 奇诺栗鼠` 标题位置。支持 `No.573`、`No. 573`、`#573` 和全角字符。没有有效文字的选区不显示标签。
4. 点击“开始识别”，切回游戏。每个选区独立轮询，两次连续结果一致后，在该选区上方显示**第一个弱点**，例如 `岩石×4`；没有属性弱点时显示“无弱点”。成功后继续识别，换敌人会更新。
5. 鼠标悬停任一小标签，展开该敌人的完整卡片，显示名字、编号、属性、全部弱点、抗性与免疫。鼠标可以移入卡片查看，离开后自动收起；浮窗不抢游戏焦点。屏幕顶部放不下标签时，改放选区旁边。
6. 主窗口下拉列表可切换查看某一选区的识别状态，支持删除当前选区或清空。选中后直接修改下拉框文字，点击“保存名称”，例如“单人敌方”“群怪左侧”“图鉴编号”；名称与选区一起保存。识别过程中添加／删除会自动重新开始轮询；手动查询不会停止其他选区识别。
7. 选区、当前选择和主窗口位置默认自动保存，下次启动恢复；点击“开始识别”即可继续。选区固定在屏幕位置；移动或缩放游戏／模拟器窗口后，用调整按钮重新定位。旧版使用游戏窗口相对坐标，升级后需重新框选一次；原 `regions.txt`／`roi.txt` 保留，沿用可读取的主窗口位置。

识别最多使用两个工作线程轮询，避免每个选区都启动并行引擎；每轮结束等待约一秒，单选区频率约一秒加 OCR 耗时，多选区每个位置的更新间隔相应增加。每个 Tesseract 子进程限制一个 OpenMP 线程。

自动 OCR 只把有 `No`／`#` 前缀的数字作为图鉴编号，**不会把 `Lv.29` 或裸数字 `573` 当作编号**。编号与可识别名字同时存在时交叉核对，冲突就清除提示。没有编号时匹配完整中文物种名，允许字间空格。

实时识别首先保留灰度笔画并进行 4 倍双线性放大、加白边，未匹配时尝试 Otsu 二值化的备用读法，同时为 Tesseract 提供本地物种词表。没有使用错字表直接替换 OCR 结果。

若仍未识别且存在唯一的同长度、仅一个汉字不同的物种，会显示候选确认按钮。例如“君主尼”只作为“君主蛇”的候选，不会自动显示它的属性。点击确认后，仅在本次运行中记住这个读法；编号冲突不会通过候选确认自动放行。多个候选时不猜测。两次读法指向不同物种时也拒绝展示结果。

屏幕选区内名字消失、匹配冲突、截图／OCR 失败时清除旧提示。切换应用后仍会识别这些屏幕位置，不判断前台应用属于哪个游戏；需要停止时点击暂停。画面被其他窗口盖住后，只能识别当前可见内容。主窗口挡住识别区时清除该选区提示，移开或使用标题栏最小化即可。展开卡片时尽量避开所有选区；如果仍有遮挡，暂缓读取被遮挡位置，最长 15 秒未更新会清除其标签，避免将辅助窗口文字当成游戏文字。

识别状态行显示原始 OCR 结果，方便排查。英文名、昵称和玩家自定义昵称暂不支持；群怪需要分别框选各自名字，不能将多个名字框在同一选区。遇到识别不到，先调整框选区域或放大游戏 UI，而不是让程序猜物种。

## 数据与属性规则

- 覆盖全国编号 **1–649**，基础形态。
- 中文名来自本机中文字符串表，基础属性来自本机《黑／白1》ROM 的 personal 数据。由 `scripts/import_ocr_species.py` 提取成事实数据 TSV，未打包 ROM、图片、对话或游戏代码。不是联网从当前 PokeMMO 客户端获取的最新属性。
- 采用第二至第五世代的 17 属性相克表，钢仍抵抗幽灵和恶，没有妖精属性。PokeMMO 社区[属性表项目说明](https://forums.pokemmo.com/index.php?/topic/201443-pc-better-default-a-cleaner-more-polished-pokemmo-theme/)也采用这一规则；若游戏后续调整，需更新本地表。
- 计算只包含基础形态属性，不判断特性、道具、招式和对战中属性改变。洛托姆、结草贵妇等不同属性形态不能仅靠基础名称或全国编号可靠区分，本版不宣称识别实际形态。

数据提取复现（开发用，运行辅助程序不需要 ROM）：

```bash
python scripts/import_ocr_species.py --rom /mnt/d/games/PokeMMO/roms/黑白.nds --strings /mnt/d/games/PokeMMO/data/strings/z_yibu_strings_zh_dump.xml
```

## 命令行诊断

```powershell
.\windows-ocr\target\release\pokemmo-hint.exe --text 'No.573 奇诺栗鼠'
.\windows-ocr\target\release\pokemmo-hint.exe --text '523'
.\windows-ocr\target\release\pokemmo-hint.exe --image 'D:\图片\敌人名字.png'
```

手动 `--text` 和窗口测试输入框支持裸编号。`--image` 对指定图像直接执行单行 OCR，建议提供已经裁剪、放大的名字／编号图片；实时截图模式会自动放大、反色。指定引擎和模型目录可使用进程环境变量 `POKEMMO_TESSERACT`、`POKEMMO_TESSDATA`。

程序直接截取屏幕选区，不枚举、打开或查询游戏进程，不点击游戏、不自动换人。只在按钮悬停提示中判断辅助程序自身是否位于前台。屏幕选区、名称与主窗口位置保存在 `%LOCALAPPDATA%\PokeMMOHint\screen-regions.txt`；旧 `regions.txt`／`roi.txt` 不覆盖、不当作屏幕坐标读取。清空选区也会保存，不会在下次启动恢复旧选区。OCR 的主／备用截图和生成的物种词表保存在程序自己的系统临时目录中，暂停／正常退出时删除；异常退出可能残留临时目录。候选确认仅保存在内存，不写入物种数据或游戏文件。

## 验证与待测

开发环境执行：

```bash
cargo test --manifest-path windows-ocr/Cargo.toml --offline
cargo check --manifest-path windows-ocr/Cargo.toml --offline --features windows-check
cargo run --manifest-path windows-ocr/Cargo.toml --offline -- --text 'No.573 奇诺栗鼠'
```

覆盖屏幕坐标保存、负坐标显示器、布局变化拒绝、旧窗口坐标格式拒绝，以及双属性相乘、四倍弱点、免疫、钢的旧世代规则、编号／名字冲突、等级数字排除、切换敌人清除旧结果、框选边界、灰度／极性处理、候选确认和不同读法冲突，以及多选区配置保存／迁移／清空、独立连续更新、标签与卡片位置。Windows 类型检查不等于新版 Windows 编译或运行已通过。

用户截图回归：用 Linux Tesseract C API 和现有中文模型，比较同一个紧凑的名字裁剪 `(140,84)-(215,106)`，旧预处理得到“君主性”，新的 Rust 灰度预处理得到“君主蛇”。另两个裁剪中也读出正确名字（可能带额外符号），完整记录见 [OCR 回归证据](../docs/evidence/ocr-serperior-regression.json)。这是单张截图的样例验证，不代表总体准确率，也不是 Windows 新版界面的实测。

Windows 新版验收：不启动 PokeMMO，在模拟器或图鉴页面添加多个屏幕位置并开始识别；各位置只显示自己的首个弱点；悬停展开对应完整卡片，移开收起且游戏不失焦；一处敌人消失不影响其他位置；更换敌人后持续更新；主窗口最小化后标签仍工作；重启恢复全部选区；清空后重启不恢复旧选区。实际小字号 OCR 准确率仍待实测。

## 应用图标

图标由内置 imagegen 生成，采用精灵球、扫描放大镜与闪电元素。原图位于 `assets/app-icon.png`，Windows 图标位于 `assets/app.ico`，包含 16／24／32／48／64／128／256 像素透明图层。完整提示词和生成方式记录在 [generation.json](assets/generation.json)。

`build.rs` 自动将 ICO 写成 Windows 图标资源并嵌入 EXE，MSVC 构建不需要额外 Rust crate 或资源编译器；GNU Windows 目标需可用的 `windres`，也可通过 `WINDRES` 指定路径。程序加载同一个内嵌资源用于标题栏与任务栏。构建不需要调用生图或安装 Pillow，图标资源已包含在项目中。

工具栏图标来自一次内置 imagegen 生成的统一风格九宫格，原图为 `assets/toolbar-sheet.png`，拆分的多尺寸 ICO 在 `assets/buttons/`，完整提示词记录在 [toolbar-generation.json](assets/toolbar-generation.json)。播放／暂停图标随识别状态切换，查询与候选确认也使用同一套图标。标题栏的系统窗口按钮沿用 Windows 样式。

## 五个 ROM 的覆盖核对

已实际读取本机火红、绿宝石、心金、白金、黑白的基础属性表：火红／绿宝石各覆盖全国编号 1–386，心金／白金各为 1–493，黑白为 1–649。五者去重仍为 649 个基础物种；与应用当前表逐条对比，缺失编号和基础属性差异均为零。客户端中文基础名称也全部吻合。查看 [完整核对记录](../docs/evidence/species-coverage-audit.json)。

黑白的 personal 档案有 669 个内部记录，但包含占位和不同形态，不代表 669 个全国编号。本机中文字符串编号 150650／150651 为蛋，150657 等为洛托姆形态；这些偏移既不是全国编号，也不能直接与 ROM 中同索引的记录对应。全国编号与基础种类完整不等于所有形态已覆盖，或这些物种在 PokeMMO 都可捕获。

目前尚未分别查询的属性变化形态包括洛托姆家电、结草贵妇蓑衣、谢米天空形态、达摩狒狒达摩模式、飘浮泡泡天气形态、美洛耶塔舞步形态等；只凭基础名或编号无法可靠区分。该限制仍存在，本次没有把它们错误追加为全国编号 650 以后。

复现核对：

```bash
python scripts/audit_ocr_coverage.py --rom-dir /mnt/d/games/PokeMMO/roms --strings /mnt/d/games/PokeMMO/data/strings/z_yibu_strings_zh_dump.xml
```

PokeMMO 对 ROM 的官方用途说明见 [Installing The Game](https://support.pokemmo.com/knowledgebase/article/installing-the-game) 和 [Importing Or Changing ROMs](https://support.pokemmo.com/knowledgebase/article/importing-or-changing-roms)。以上物种数量来自本机实际解析，不把社区建议帖当成已上线的游戏功能。

## 发版与用户安装

在 Windows 仓库根目录执行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_windows.ps1
```

输出为 `dist/PokeMMO-Hint-v版本-windows-x64.zip` 及 `.zip.sha256`。发给用户的是这两个文件；用户解压整个 ZIP 后双击 `Start.cmd`，不需要 Rust，包内已有 Tesseract、运行 DLL 和中文模型，无需联网安装或管理员权限。详见 [发布流程](../docs/releasing.md)。
