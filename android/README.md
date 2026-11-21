# 宝可梦属性提示 Android

支持 Android 8.0 及以上。通过系统录屏和悬浮窗读取文字选区，画面识别在本机完成。界面为简体中文，游戏文字支持简体中文、英语和日语。使用随包的 ML Kit 文字识别模型，运行时无需下载模型或安装 Google Play 服务。

1. 打开应用并选择游戏文字语言、识别刷新率。默认目标每秒 1 次，较慢时可减少选区。
2. 点击「开启悬浮识别」，允许显示在其他应用上层后返回应用，再开启并确认共享整个屏幕。授权后应用退到后台。
3. 切回游戏，点 Logo 悬浮球展开，再点「＋」，手指拖框只圈一行物种名或 `No.` 编号。可命名并保存，最多 16 个选区。横屏和竖屏各自保存。
4. 连续两轮确认后，选区旁显示带三角尖的弱点气泡。点按展开详情，查看弱点、普通伤害、抗性和免疫。换敌人后结果会更新。
5. 「☰」管理选区，可以调整、重画、改名或删除。拖动 Logo 可以移动悬浮球，松手时靠近屏幕边缘会吸附为半球。约 4 秒无操作会收起并吸附到最近的屏幕边缘，初始位置靠右。收起后的 Logo 不透明度为 80%。「Ⅱ」暂停，「×」结束并释放录屏。
6. 主页可手动查中英日名称或编号，或打开最近 20 条识别记录。修改游戏文字语言后，结束并重新开启识别即可切换引擎。

图鉴在构建时复用 Windows 的全国编号 1–649 数据和第二至第五世代相克规则。录屏授权随会话结束而失效。若目标游戏画面受系统录屏保护或禁止悬浮窗，屏幕识别功能可能无法使用，仍可手动查询。

## 构建安装

使用 Android Studio 打开本目录，同步项目后运行 `app`。命令行需要 JDK 17、Android SDK 35 和能访问 Google Maven 与 Maven Central 的网络。

```powershell
cd android
.\gradlew.bat :app:assembleDebug

adb install .\app\build\outputs\apk\debug\app-debug.apk
```

Linux/macOS 使用 `bash ./gradlew :app:assembleDebug`。调试 APK 路径为 `app/build/outputs/apk/debug/app-debug.apk`。正式分发需要使用自己的签名密钥构建。

[Android MediaProjection](https://developer.android.com/media/grow/media-projection) · [ML Kit 文字识别](https://developers.google.com/ml-kit/vision/text-recognition/v2/android)
