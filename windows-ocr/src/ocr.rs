use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct OcrConfig {
    pub executable: PathBuf,
    pub models: PathBuf,
}
impl OcrConfig {
    pub fn discover() -> Result<Self, String> {
        let base = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .unwrap()
            .to_path_buf();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut candidates = Vec::new();
        if let Some(path) = std::env::var_os("POKEMMO_TESSERACT") {
            candidates.push(PathBuf::from(path));
        }
        candidates.push(base.join("ocr/tesseract.exe"));
        for key in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(path) = std::env::var_os(key) {
                candidates.push(PathBuf::from(path).join("Tesseract-OCR/tesseract.exe"));
            }
        }
        if let Some(paths) = std::env::var_os("PATH") {
            for path in std::env::split_paths(&paths) {
                candidates.push(path.join(if cfg!(windows) {
                    "tesseract.exe"
                } else {
                    "tesseract"
                }));
            }
        }
        let executable=candidates.into_iter().find(|p|p.is_file()).ok_or(
            "未找到 Tesseract。发行包请运行 Start.cmd，源码版请运行 windows-ocr/setup.ps1 -InstallOcr；或设置 POKEMMO_TESSERACT 为 tesseract.exe 路径")?;
        let model_dirs = [
            std::env::var_os("POKEMMO_TESSDATA").map(PathBuf::from),
            Some(base.join("ocr/tessdata")),
            Some(root.join("ocr/tessdata")),
            executable.parent().map(|p| p.join("tessdata")),
        ];
        let models = model_dirs
            .into_iter()
            .flatten()
            .find(|p| p.join("chi_sim.traineddata").is_file())
            .ok_or("未找到简体中文模型 chi_sim.traineddata，请完整解压发行包，或在源码版运行 windows-ocr/setup.ps1")?;
        Ok(Self { executable, models })
    }

    pub fn recognize(&self, image: &Path, cancelled: &AtomicBool) -> Result<String, String> {
        self.recognize_with_words(image, cancelled, None)
    }

    pub fn recognize_with_words(
        &self,
        image: &Path,
        cancelled: &AtomicBool,
        words: Option<&Path>,
    ) -> Result<String, String> {
        let mut command = Command::new(&self.executable);
        command
            .env("OMP_THREAD_LIMIT", "1")
            .arg(image)
            .args(["stdout", "--tessdata-dir"])
            .arg(&self.models)
            .args(["-l", "chi_sim", "--oem", "1", "--psm", "7", "--dpi", "300"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(words) = words {
            command.arg("--user-words").arg(words);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let mut child = command.spawn().map_err(|e| format!("启动 OCR 失败：{e}"))?;
        // Drain pipes concurrently: diagnostic stderr must not deadlock the child.
        use std::io::Read;
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        fn drain(mut pipe: impl Read) -> Vec<u8> {
            let mut b = Vec::new();
            let _ = pipe.read_to_end(&mut b);
            b
        }
        let out = std::thread::spawn(move || drain(stdout));
        let err = std::thread::spawn(move || drain(stderr));
        let start = Instant::now();
        let status = loop {
            if cancelled.load(Ordering::Relaxed) || start.elapsed() > Duration::from_secs(5) {
                let _ = child.kill();
                let _ = child.wait();
                let _ = out.join();
                let _ = err.join();
                return Err("OCR 已停止或超时（5 秒）".to_owned());
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => std::thread::sleep(Duration::from_millis(40)),
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = out.join();
                    let _ = err.join();
                    return Err(e.to_string());
                }
            }
        };
        let output = out.join().map_err(|_| "OCR 输出读取失败")?;
        let error = err.join().map_err(|_| "OCR 错误读取失败")?;
        if !status.success() {
            return Err(format!(
                "OCR 失败：{}",
                String::from_utf8_lossy(&error).trim()
            ));
        }
        String::from_utf8(output)
            .map(|s| s.trim().to_owned())
            .map_err(|_| "OCR 输出不是 UTF-8".to_owned())
    }
}
