#![cfg_attr(windows, windows_subsystem = "windows")]

use pokemmo_hint::{hint, lookup_query, match_ocr, species_data, Language, MatchResult};

#[cfg(any(windows, feature = "windows-check"))]
mod ocr;
#[cfg(any(windows, feature = "windows-check"))]
mod ocr_process;
#[cfg(any(windows, feature = "windows-check"))]
mod onnx;
#[cfg(any(windows, feature = "windows-check"))]
mod win;

fn main() {
    let language = cli_language();
    if let Err(error) = run() {
        #[cfg(windows)]
        if std::env::args_os().len() == 1 {
            win::show_startup_error(&error);
        }
        let prefix = match language {
            Language::Zh => "错误：",
            Language::En => "Error: ",
            Language::Ja => "エラー: ",
        };
        eprintln!("{prefix}{error}");
        std::process::exit(1);
    }
}

fn cli_language() -> Language {
    let args: Vec<_> = std::env::args().skip(1).collect();
    args.iter()
        .position(|arg| arg == "--lang")
        .and_then(|index| args.get(index + 1))
        .and_then(|value| Language::parse(value))
        .unwrap_or(Language::Zh)
}

fn usage(language: Language) -> String {
    match language {
        Language::Zh => "用法：pokemmo-hint [--lang zh|en|ja] [--game-lang zh|en|ja] [--text 名字或编号 | --image 图片路径]；不带参数打开 Windows 窗口",
        Language::En => "Usage: pokemmo-hint [--lang zh|en|ja] [--game-lang zh|en|ja] [--text name-or-number | --image path]; run without arguments to open the Windows window",
        Language::Ja => "使い方: pokemmo-hint [--lang zh|en|ja] [--game-lang zh|en|ja] [--text 名前または番号 | --image 画像パス]。引数なしで Windows ウィンドウを開きます",
    }
    .to_owned()
}

fn localized(language: Language, zh: &str, en: &str, ja: &str) -> String {
    match language {
        Language::Zh => zh,
        Language::En => en,
        Language::Ja => ja,
    }
    .to_owned()
}

fn run() -> Result<(), String> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut language = Language::Zh;
    let mut game_language = None;
    let mut args = Vec::new();
    let mut index = 0;
    while index < raw.len() {
        match raw[index].as_str() {
            "--lang" | "--game-lang" => {
                let flag = raw[index].clone();
                let value = raw.get(index + 1).ok_or_else(|| usage(language))?;
                let parsed = Language::parse(value).ok_or_else(|| {
                    localized(
                        language,
                        "语言参数必须是 zh、en 或 ja",
                        "Language must be zh, en, or ja.",
                        "言語は zh、en、ja のいずれかです。",
                    )
                })?;
                if flag == "--lang" {
                    language = parsed;
                } else {
                    game_language = Some(parsed);
                }
                index += 2;
            }
            _ => {
                args.push(raw[index].clone());
                index += 1;
            }
        }
    }
    let game_language = game_language.unwrap_or(language);
    if let Some(flag) = args.first() {
        #[cfg(any(windows, feature = "windows-check"))]
        if flag == "--paddle-image" || flag == "--paddle-check" {
            let checking = flag == "--paddle-check";
            let dir = args
                .get(if checking { 1 } else { 2 })
                .ok_or("缺少模型目录")?;
            let session = onnx::Session::load(std::path::Path::new(dir))?;
            if checking {
                session.check()?;
                println!("PADDLE_READY");
            } else {
                for image in [args.get(1), args.get(3)].into_iter().flatten() {
                    match session.recognize(std::path::Path::new(image)) {
                        Ok(text) => println!("{text}"),
                        Err(error) if error.contains("置信度不足") => {}
                        Err(error) => return Err(error),
                    }
                }
            }
            return Ok(());
        }

        if flag == "--text" || flag == "--image" {
            let value = args.get(1).ok_or_else(|| {
                localized(
                    language,
                    "需要提供文字或图片路径",
                    "A name, number, or image path is required.",
                    "文字または画像パスを指定してください。",
                )
            })?;
            let data = species_data();
            let result = if flag == "--image" {
                let text = recognize_image(value, game_language).map_err(|error| {
                    if error == "PADDLE_NOT_READY" {
                        localized(
                            language,
                            "请打开设置，选择 PP-OCR 并完成下载和检测。",
                            "Open settings, choose PP-OCR, and finish the download and check.",
                            "設定で PP-OCR を選び、ダウンロードと検証を完了してください。",
                        )
                    } else if error.contains("WINDOWS_OCR_UNAVAILABLE") {
                        match language {
                            Language::Zh => format!(
                                "系统{} OCR 不可用；请安装该系统语言，或改用 No.编号。",
                                game_language.native_name()
                            ),
                            Language::En => format!(
                                "System {} OCR is unavailable; install that Windows language, or use a No. number.",
                                game_language.native_name()
                            ),
                            Language::Ja => format!(
                                "システムの{} OCR を利用できません。その言語をインストールするか、No. 番号をご利用ください。",
                                game_language.native_name()
                            ),
                        }
                    } else {
                        error
                    }
                })?;
                println!("OCR: {text}");
                match_ocr(&text, game_language, &data)
            } else {
                lookup_query(value, game_language, &data)
            };
            match result {
                MatchResult::Found(id) => {
                    println!("{}", hint(data.iter().find(|s| s.id == id).unwrap(), language))
                }
                MatchResult::Unknown => {
                    return Err(localized(
                        language,
                        "未匹配；请输入所选游戏语言的物种名或全国编号",
                        "No match. Enter a species name in the selected game language or a National No.",
                        "一致しません。選択したゲーム言語の種族名または全国図鑑番号を入力してください。",
                    ))
                }
                MatchResult::Ambiguous => {
                    return Err(localized(
                        language,
                        "包含多个物种名，请缩小识别区域",
                        "Multiple species names found; narrow the region.",
                        "複数の種族名があります。範囲を狭めてください。",
                    ))
                }
                MatchResult::Conflict => {
                    return Err(localized(
                        language,
                        "编号与物种名冲突，请重新识别",
                        "The number and species name conflict. Try again.",
                        "番号と種族名が一致しません。もう一度認識してください。",
                    ))
                }
            }
            return Ok(());
        }
        return Err(usage(language));
    }
    #[cfg(windows)]
    {
        win::run()
    }
    #[cfg(not(windows))]
    {
        Err(localized(
            language,
            "界面和截图仅支持 Windows；本平台可用 --text 验证匹配和属性计算",
            "The window and screen capture require Windows; use --text here to check matching and type data.",
            "ウィンドウと画面キャプチャは Windows 専用です。ここでは --text で照合とタイプ計算を確認できます。",
        ))
    }
}

#[cfg(any(windows, feature = "windows-check"))]
fn recognize_image(value: &str, game_language: Language) -> Result<String, String> {
    let script = std::env::temp_dir().join(format!(
        "pokemon-cli-{}-windows-ocr.ps1",
        std::process::id()
    ));
    std::fs::write(&script, include_str!("windows_ocr.ps1")).map_err(|e| e.to_string())?;
    let mut settings = pokemmo_hint::engines::load_settings()?;
    settings.game_language = game_language;
    let result = (|| {
        ocr::Backend::selected(settings)?.recognize(
            std::path::Path::new(value),
            None,
            &script,
            &std::sync::atomic::AtomicBool::new(false),
        )
    })();
    let _ = std::fs::remove_file(script);
    result
}
#[cfg(not(any(windows, feature = "windows-check")))]
fn recognize_image(_value: &str, _game_language: Language) -> Result<String, String> {
    Err("图片识别仅支持 Windows".to_owned())
}
