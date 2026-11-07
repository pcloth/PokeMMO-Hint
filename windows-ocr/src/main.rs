use pokemmo_hint::{hint, lookup_query, match_ocr, species_data, MatchResult};

mod ocr;
#[cfg(any(windows, feature = "windows-check"))]
mod win;

fn main() {
    if let Err(error) = run() {
        eprintln!("错误：{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if let Some(flag) = args.first() {
        if flag == "--text" || flag == "--image" {
            let value = args.get(1).ok_or("需要提供文字或图片路径")?;
            let data = species_data();
            let result = if flag == "--image" {
                let config = ocr::OcrConfig::discover()?;
                let text = config.recognize(
                    std::path::Path::new(value),
                    &std::sync::atomic::AtomicBool::new(false),
                )?;
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
