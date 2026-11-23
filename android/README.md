# 宝可梦属性提示 Android

支持 Android 8.0 及以上。通过系统录屏和悬浮窗读取文字选区，画面识别在本机完成。界面为简体中文，游戏文字支持简体中文、英语和日语。使用随包的 ML Kit 文字识别模型，运行时无需下载模型或安装 Google Play 服务。

1. 打开应用并选择游戏文字语言、识别刷新率。默认目标每秒 1 次，较慢时可减少选区。
2. 点击「开启悬浮识别」，允许显示在其他应用上层后返回应用，再开启并确认共享整个屏幕。授权后应用退到后台。
3. 切回游戏，点 Logo 悬浮球展开，再点「＋」，手指拖框只圈一行物种名或 `No.` 编号。可命名并保存，最多 16 个选区。横屏和竖屏各自保存。
4. 连续两轮确认后，选区旁显示带三角尖的弱点气泡。点按展开详情，查看弱点、普通伤害、抗性和免疫。换敌人后结果会更新。
5. 「☰」管理选区，可以调整、重画、改名或删除。拖动 Logo 可以移动悬浮球，松手时靠近屏幕边缘会吸附为半球。约 4 秒无操作会收起并吸附到最近的屏幕边缘，初始位置靠右。收起后的 Logo 不透明度为 80%。「Ⅱ」暂停，「×」结束并释放录屏。
6. 主页可手动查询中英日名称或全国编号 001–1025。查询历史保存最近 20 个 OCR 识别或手动查询成功的物种。点击记录可重新查看，即使当前处于 PokeMMO 模式，也能打开苍响等现代物种。修改游戏文字语言后，重新开启识别即可切换引擎。

默认图鉴在构建时复用 Windows 的全国编号 1–649 数据和第二至第五世代相克规则。录屏授权随会话结束而失效。若目标游戏画面受系统录屏保护或禁止悬浮窗，屏幕识别功能可能无法使用，仍可手动查询。

## 属性标签页

主界面的查询结果和游戏中的悬浮详情卡都支持「PokeMMO」「现代」「太晶化」「极巨化」四个标签页。齿轮设置中的「现代图鉴」决定默认打开的标签页，以及 OCR 使用的物种范围。

- 「PokeMMO」和「现代」各自显示完整属性相克。可在「现代」页选择不同形态。
- 「太晶化」可选择当前太晶属性，星晶保留太晶化前的防守属性。选择厄诡椪的面具后，太晶属性会随面具调整。
- 「极巨化」页只适用于《剑／盾》可极巨化的物种，支持符合条件的超极巨化形态。两者都保留原形态的防守属性。

切换悬浮详情卡的标签页或形态后，选区的弱点气泡会同步更新。遇到不同物种时会重置该选区的选择。

Android 与 Windows 共享同一份现代物种、形态和倍率数据。《明耀之星》可按名称查物种的基础属性弱点。

## 构建安装

使用 Android Studio 打开本目录，同步项目后运行 `app`。命令行需要 JDK 17、Android SDK 35 和能访问 Google Maven 与 Maven Central 的网络。

```powershell
cd android
.\gradlew.bat :app:assembleDebug

adb install .\app\build\outputs\apk\debug\app-debug.apk
```

Linux/macOS 使用 `bash ./gradlew :app:assembleDebug`。调试 APK 路径为 `app/build/outputs/apk/debug/app-debug.apk`。

## 打包发行版

在仓库根目录打开 PowerShell。构建前在 `android/app/build.gradle` 更新 `versionName`，并将 `versionCode` 提高到已发布版本之上。以下命令使用 JDK 17 和 Android SDK Build-Tools 35.0.0。若 SDK 装在其他位置，请修改 `$sdk`。

首次发布时创建签名密钥。已有本应用的发行密钥时，跳过生成命令，并在后续命令中填写原密钥路径和别名。后续版本覆盖安装需要使用相同的应用 ID 和签名密钥。请备份密钥库和密码，并存放在仓库外。

```powershell
$key = Join-Path $env:USERPROFILE 'pokemon-hint-release.p12'
$alias = 'pokemon-hint'
keytool -genkeypair -storetype PKCS12 -keystore $key -alias $alias -keyalg RSA -keysize 2048 -validity 10000
```

打包、对齐、签名并验证。签名时按提示输入密码。

```powershell
$key = Join-Path $env:USERPROFILE 'pokemon-hint-release.p12'
$alias = 'pokemon-hint'
$sdk = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { Join-Path $env:LOCALAPPDATA 'Android\Sdk' }
$tools = Join-Path $sdk 'build-tools\35.0.0'

cd android
.\gradlew.bat :app:assembleRelease
if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }

$unsigned = '.\app\build\outputs\apk\release\app-release-unsigned.apk'
$aligned = '.\app\build\outputs\apk\release\app-release-aligned.apk'
$release = '..\dist\pokemon-hint-android-release.apk'
New-Item -ItemType Directory -Force '..\dist' | Out-Null

& (Join-Path $tools 'zipalign.exe') -P 16 -f 4 $unsigned $aligned
if ($LASTEXITCODE -ne 0) { throw 'APK alignment failed' }
& (Join-Path $tools 'apksigner.bat') sign --ks $key --ks-key-alias $alias --out $release $aligned
if ($LASTEXITCODE -ne 0) { throw 'APK signing failed' }
& (Join-Path $tools 'apksigner.bat') verify --verbose --print-certs $release
if ($LASTEXITCODE -ne 0) { throw 'APK signature verification failed' }

adb install -r $release
```

**安装和分发都使用 `dist/pokemon-hint-android-release.apk`。** `app-release-unsigned.apk` 和 `app-release-aligned.apk` 是未签名的中间文件，安装时会报 `INSTALL_PARSE_FAILED_NO_CERTIFICATES`。

若当前目录是 `android/app/build/outputs/apk/release`，可直接运行以下命令安装已签名的包。

```powershell
adb install -r ..\..\..\..\..\..\dist\pokemon-hint-android-release.apk
```

若手机已安装调试包，或装的是用另一把密钥签名的版本，Android 会拒绝覆盖安装。卸载旧包会清除本地选区、设置和历史。

[Android APK 签名工具](https://developer.android.com/tools/apksigner) · [APK 对齐工具](https://developer.android.com/tools/zipalign)

[Android MediaProjection](https://developer.android.com/media/grow/media-projection) · [ML Kit 文字识别](https://developers.google.com/ml-kit/vision/text-recognition/v2/android)
