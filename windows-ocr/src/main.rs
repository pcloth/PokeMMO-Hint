#![cfg_attr(windows, windows_subsystem = "windows")]

use pokemmo_hint::{hint, lookup_query, match_ocr, species_data, MatchResult};

#[cfg(any(windows, feature = "windows-check"))]
mod ocr;
#[cfg(any(windows, feature = "windows-check"))]
mod ocr_process;
#[cfg(any(windows, feature = "windows-check"))]
mod onnx;
#[cfg(any(windows, feature = "windows-check"))]
mod win;

fn main() {
    if let Err(error) = run() {
        #[cfg(windows)]
        if std::env::args_os().len() == 1 {
            win::show_startup_error(&error);
        }
        eprintln!("错误：{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
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
            let value = args.get(1).ok_or("需要提供文字或图片路径")?;
            let data = species_data();
            let result = if flag == "--image" {
                let text = recognize_image(value)?;
                println!("OCR: {text}");
                match_ocr(&text, &data)
            } else {
                lookup_query(value, &data)
            };
            match result {
                MatchResult::Found(id) => {
                    println!("{}", hint(data.iter().find(|s| s.id == id).unwrap()))
                }
                MatchResult::Unknown => {
                    return Err("未匹配；当前只支持中文基础物种名或手动输入全国编号".to_owned())
                }
                MatchResult::Ambiguous => return Err("包含多个物种名，请缩小识别区域".to_owned()),
                MatchResult::Conflict => return Err("编号与物种名冲突，请重新识别".to_owned()),
            }
            return Ok(());
        }
        return Err(
            "用法：pokemmo-hint [--text 名字或编号 | --image 图片路径]；不带参数打开 Windows 窗口"
                .to_owned(),
        );
    }
    #[cfg(windows)]
    {
        win::run()
    }
    #[cfg(not(windows))]
    {
        Err("界面和截图仅支持 Windows；本平台可用 --text 验证匹配和属性计算".to_owned())
    }
}

#[cfg(any(windows, feature = "windows-check"))]
fn recognize_image(value: &str) -> Result<String, String> {
    let script = std::env::temp_dir().join(format!(
        "pokemon-cli-{}-windows-ocr.ps1",
        std::process::id()
    ));
    std::fs::write(&script, include_str!("windows_ocr.ps1")).map_err(|e| e.to_string())?;
    let result = (|| {
        ocr::Backend::selected(pokemmo_hint::engines::load_settings()?)?.recognize(
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
fn recognize_image(_value: &str) -> Result<String, String> {
    Err("图片识别仅支持 Windows".to_owned())
}
