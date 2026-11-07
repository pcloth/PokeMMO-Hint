use std::path::PathBuf;

pub const MODEL_SHA256: &str = "9ef676d6ed3c88256a2d92c640c44f25b0c40947e111b14b8be8f594091563e6";
pub const MODEL_URL: &str = "PaddlePaddle/PP-OCRv6_tiny_rec_onnx/resolve/main/inference.onnx";
pub const DICTIONARY: &str = include_str!("../data/ppocrv6_tiny_dict.txt");
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Engine {
    #[default]
    Windows,
    PaddleTiny,
}
impl Engine {
    pub fn label(self) -> &'static str {
        match self {
            Self::Windows => "Windows 中文 OCR",
            Self::PaddleTiny => "PP-OCRv6 tiny",
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DownloadSource {
    #[default]
    Mirror,
    Official,
}
pub const REFRESH_INTERVALS_MS: [u64; 4] = [2000, 1000, 500, 250];
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineSettings {
    pub engine: Engine,
    pub source: DownloadSource,
    pub refresh_interval_ms: u64,
}
impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            engine: Engine::Windows,
            source: DownloadSource::Mirror,
            refresh_interval_ms: 1000,
        }
    }
}
impl EngineSettings {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        if lines.next() != Some("pokemon-ocr-engines-v1") {
            return Err("识别设置格式错误".to_owned());
        }
        let mut value = Self::default();
        for line in lines.filter(|s| !s.is_empty()) {
            let (k, v) = line.split_once('=').ok_or("识别设置格式错误")?;
            match (k, v) {
                ("engine", "windows") => value.engine = Engine::Windows,
                ("engine", "ppocrv6-tiny") => value.engine = Engine::PaddleTiny,
                ("source", "mirror") => value.source = DownloadSource::Mirror,
                ("source", "official") => value.source = DownloadSource::Official,
                ("refresh_interval_ms", text) => {
                    let ms = text.parse::<u64>().map_err(|_| "识别刷新率格式错误")?;
                    if !REFRESH_INTERVALS_MS.contains(&ms) {
                        return Err("不支持的识别刷新率".to_owned());
                    }
                    value.refresh_interval_ms = ms;
                }
                _ => return Err("未知识别设置".to_owned()),
            }
        }
        Ok(value)
    }
    pub fn encode(self) -> String {
        format!(
            "pokemon-ocr-engines-v1\nengine={}\nsource={}\nrefresh_interval_ms={}\n",
            if self.engine == Engine::Windows {
                "windows"
            } else {
                "ppocrv6-tiny"
            },
            if self.source == DownloadSource::Mirror {
                "mirror"
            } else {
                "official"
            },
            self.refresh_interval_ms
        )
    }
    pub fn model_urls(self) -> [String; 2] {
        let mirror = format!("https://hf-mirror.com/{MODEL_URL}");
        let official = format!("https://huggingface.co/{MODEL_URL}");
        if self.source == DownloadSource::Mirror {
            [mirror, official]
        } else {
            [official, mirror]
        }
    }
}
pub fn storage_dir() -> Result<PathBuf, String> {
    std::env::var_os("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("PokeMMOHint"))
        .ok_or("无法取得本机应用数据目录".to_owned())
}
pub fn assets_dir() -> Result<PathBuf, String> {
    Ok(storage_dir()?.join("ocr/ppocrv6-tiny-9ef676d6"))
}
/// A successful prior preparation can be reused without starting the downloader.
/// The inference helper still loads the cached DLL/model before enabling Apply.
pub fn cached_assets_ready(dir: &std::path::Path) -> bool {
    std::fs::read_to_string(dir.join("ready.txt")).is_ok_and(|marker| marker.trim() == MODEL_SHA256)
        && ["inference.onnx", "onnxruntime.dll"]
            .iter()
            .all(|name| std::fs::metadata(dir.join(name)).is_ok_and(|m| m.is_file() && m.len() > 0))
}
pub fn load_settings() -> Result<EngineSettings, String> {
    match std::fs::read_to_string(storage_dir()?.join("ocr-settings.txt")) {
        Ok(text) => EngineSettings::parse(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(EngineSettings::default()),
        Err(e) => Err(e.to_string()),
    }
}
pub fn save_settings(value: EngineSettings) -> Result<(), String> {
    let dir = storage_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let temp = dir.join("ocr-settings.tmp");
    std::fs::write(&temp, value.encode()).map_err(|e| e.to_string())?;
    std::fs::rename(temp, dir.join("ocr-settings.txt")).map_err(|e| e.to_string())
}
pub fn dictionary() -> Vec<String> {
    let mut values = vec![String::new()];
    values.extend(DICTIONARY.lines().map(str::to_owned));
    values.push(" ".to_owned());
    values
}

/// Paddle recognition: bilinear resize to height 48, aspect-preserving padding, BGR CHW [-1,1].
pub fn tensor_from_bmp(bmp: &[u8]) -> Result<(Vec<f32>, usize), String> {
    if bmp.len() < 54 || &bmp[..2] != b"BM" {
        return Err("PP-OCR 输入需要 32 位 BMP".to_owned());
    }
    let offset = u32::from_le_bytes(bmp[10..14].try_into().unwrap()) as usize;
    let width = i32::from_le_bytes(bmp[18..22].try_into().unwrap());
    let height = i32::from_le_bytes(bmp[22..26].try_into().unwrap());
    let bits = u16::from_le_bytes(bmp[28..30].try_into().unwrap());
    let compression = u32::from_le_bytes(bmp[30..34].try_into().unwrap());
    if width <= 0
        || width > 10000
        || height == 0
        || height.unsigned_abs() > 10000
        || bits != 32
        || compression != 0
    {
        return Err("不支持的 BMP 格式或尺寸".to_owned());
    }
    let (w, h) = (width as usize, height.unsigned_abs() as usize);
    let required = offset.checked_add(w * h * 4).ok_or("BMP 尺寸溢出")?;
    if offset < 54 || required > bmp.len() {
        return Err("BMP 数据不完整".to_owned());
    }
    let resized = ((w as f64 * 48.0 / h as f64).ceil() as usize).clamp(1, 1280);
    let padded = resized.max(320);
    let mut out = vec![0.0; 3 * 48 * padded];
    let pixel = |x: usize, y: usize, c: usize| -> f32 {
        let row = if height < 0 { y } else { h - 1 - y };
        bmp[offset + (row * w + x) * 4 + c] as f32
    };
    for y in 0..48 {
        for x in 0..resized {
            let sx =
                ((x as f64 + 0.5) * w as f64 / resized as f64 - 0.5).clamp(0.0, (w - 1) as f64);
            let sy = ((y as f64 + 0.5) * h as f64 / 48.0 - 0.5).clamp(0.0, (h - 1) as f64);
            let (x0, y0) = (sx.floor() as usize, sy.floor() as usize);
            let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
            let (tx, ty) = ((sx - x0 as f64) as f32, (sy - y0 as f64) as f32);
            for c in 0..3 {
                let a = pixel(x0, y0, c) * (1.0 - tx) + pixel(x1, y0, c) * tx;
                let b = pixel(x0, y1, c) * (1.0 - tx) + pixel(x1, y1, c) * tx;
                out[c * 48 * padded + y * padded + x] =
                    ((a * (1.0 - ty) + b * ty) / 127.5 - 1.0).clamp(-1.0, 1.0);
            }
        }
    }
    Ok((out, padded))
}
pub fn decode_ctc(scores: &[f32], steps: usize, classes: usize) -> Result<String, String> {
    let keys = dictionary();
    if classes != keys.len() && classes + 1 != keys.len() {
        return Err(format!(
            "模型字表不匹配：输出 {classes} 类，字表 {} 类",
            keys.len()
        ));
    }
    if scores.len() != steps.checked_mul(classes).ok_or("输出尺寸溢出")? {
        return Err("模型输出长度错误".to_owned());
    }
    let (mut last, mut result, mut sum, mut count) = (usize::MAX, String::new(), 0f32, 0usize);
    for row in scores.chunks_exact(classes) {
        if !row[0].is_finite() {
            return Err("模型输出不是有效概率".to_owned());
        }
        let mut best = 0;
        for i in 1..classes {
            if !row[i].is_finite() {
                return Err("模型输出不是有效概率".to_owned());
            }
            if row[i] > row[best] {
                best = i;
            }
        }
        if best != 0 && best != last {
            result.push_str(&keys[best]);
            sum += row[best];
            count += 1;
        }
        last = best;
    }
    if count > 0 && sum / (count as f32) < 0.5 {
        return Err("PP-OCR 置信度不足".to_owned());
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_and_download_sources_round_trip() {
        let a = EngineSettings {
            engine: Engine::PaddleTiny,
            source: DownloadSource::Mirror,
            refresh_interval_ms: 500,
        };
        assert_eq!(EngineSettings::parse(&a.encode()).unwrap(), a);
        assert!(a.model_urls()[0].starts_with("https://hf-mirror.com/"));
        assert_eq!(EngineSettings::default().engine, Engine::Windows);
        assert!(EngineSettings::parse("bad").is_err());
        assert_eq!(
            EngineSettings::parse("pokemon-ocr-engines-v1\nengine=windows\nsource=mirror\n")
                .unwrap()
                .refresh_interval_ms,
            1000
        );
        for ms in REFRESH_INTERVALS_MS {
            let settings = EngineSettings {
                refresh_interval_ms: ms,
                ..EngineSettings::default()
            };
            assert_eq!(EngineSettings::parse(&settings.encode()).unwrap(), settings);
        }
        assert!(EngineSettings::parse("pokemon-ocr-engines-v1\nrefresh_interval_ms=0\n").is_err());
    }
    #[test]
    fn completed_cache_survives_reopening_and_source_changes() {
        let dir = std::env::temp_dir().join(format!(
            "pokemon-cache-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        assert!(!cached_assets_ready(&dir));
        std::fs::write(dir.join("inference.onnx"), b"model").unwrap();
        std::fs::write(dir.join("onnxruntime.dll"), b"runtime").unwrap();
        assert!(
            !cached_assets_ready(&dir),
            "partial download is not a prepared cache"
        );
        std::fs::write(dir.join("ready.txt"), MODEL_SHA256).unwrap();
        for source in [DownloadSource::Mirror, DownloadSource::Official] {
            let settings = EngineSettings {
                engine: Engine::PaddleTiny,
                source,
                ..EngineSettings::default()
            };
            assert_eq!(EngineSettings::parse(&settings.encode()).unwrap(), settings);
            assert!(
                cached_assets_ready(&dir),
                "changing the download source must preserve the cache"
            );
        }
        std::fs::write(dir.join("ready.txt"), "different model version").unwrap();
        assert!(!cached_assets_ready(&dir));
        std::fs::write(dir.join("ready.txt"), MODEL_SHA256).unwrap();
        std::fs::write(dir.join("onnxruntime.dll"), b"").unwrap();
        assert!(!cached_assets_ready(&dir));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn dictionary_covers_rare_example_characters_and_ctc_preserves_repeated_names() {
        let keys = dictionary();
        assert_eq!(keys.len(), 6906);
        let chars: std::collections::BTreeSet<_> = keys.iter().flat_map(|s| s.chars()).collect();
        for c in "猫鼬斩喵蛹".chars() {
            assert!(chars.contains(&c));
        }
        let cat = keys.iter().position(|k| k == "喵").unwrap();
        let mut scores = vec![0.0; keys.len() * 4];
        for (t, c) in [(0, cat), (1, cat), (2, 0), (3, cat)] {
            scores[t * keys.len() + c] = 0.99;
        }
        assert_eq!(decode_ctc(&scores, 4, keys.len()).unwrap(), "喵喵");
        assert!(decode_ctc(&scores, 4, 2).is_err());
        scores[0] = f32::NAN;
        assert!(decode_ctc(&scores, 4, keys.len()).is_err());
        let mut low = vec![0.0; keys.len()];
        low[cat] = 0.1;
        assert!(decode_ctc(&low, 1, keys.len()).is_err());
    }
    #[test]
    fn image_tensor_has_correct_channels_padding_and_range() {
        let mut bmp = vec![0u8; 54 + 10 * 10 * 4];
        bmp[..2].copy_from_slice(b"BM");
        bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&10i32.to_le_bytes());
        bmp[22..26].copy_from_slice(&(-10i32).to_le_bytes());
        bmp[28..30].copy_from_slice(&32u16.to_le_bytes());
        for p in bmp[54..].chunks_exact_mut(4) {
            p.copy_from_slice(&[0, 128, 255, 255]);
        }
        let (t, w) = tensor_from_bmp(&bmp).unwrap();
        assert_eq!(w, 320);
        assert_eq!(t.len(), 3 * 48 * w);
        assert_eq!(t[0], -1.0);
        assert!((t[48 * w] - 0.003921628).abs() < 0.00001);
        assert_eq!(t[2 * 48 * w], 1.0);
        assert!(t.iter().all(|v| (-1.0..=1.0).contains(v)));
        assert_eq!(t[319], 0.0);
        let mut invalid = bmp.clone();
        invalid[10..14].copy_from_slice(&0u32.to_le_bytes());
        assert!(tensor_from_bmp(&invalid).is_err());
        assert!(tensor_from_bmp(&bmp[..bmp.len() - 1]).is_err());
        assert!(tensor_from_bmp(b"bad").is_err());
    }
}
