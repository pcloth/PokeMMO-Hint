use std::{path::Path, path::PathBuf, process::Command, sync::atomic::AtomicBool, time::Duration};

#[cfg(not(windows))]
use std::{process::Stdio, sync::atomic::Ordering, time::Instant};

pub struct OcrProcess;
impl OcrProcess {
    #[cfg(any(windows, feature = "windows-check"))]
    pub fn recognize_windows(
        image: &Path,
        alternate: Option<&Path>,
        script: &Path,
        cancelled: &AtomicBool,
    ) -> Result<String, String> {
        let shell = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .ok_or("WINDOWS_CHINESE_OCR_UNAVAILABLE")?
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        if !shell.is_file() {
            return Err("WINDOWS_CHINESE_OCR_UNAVAILABLE".to_owned());
        }
        let mut command = Command::new(shell);
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(script)
            .arg("-ImagePath")
            .arg(image);
        if let Some(alternate) = alternate {
            command.arg("-AlternativeImagePath").arg(alternate);
        }
        Self::capture_command(command, cancelled)
    }
    pub(crate) fn capture_command(
        command: Command,
        cancelled: &AtomicBool,
    ) -> Result<String, String> {
        Self::capture_timeout(command, cancelled, Duration::from_secs(15))
    }
    pub(crate) fn capture_timeout(
        command: Command,
        cancelled: &AtomicBool,
        timeout: Duration,
    ) -> Result<String, String> {
        #[cfg(windows)]
        {
            crate::ocr_process::capture(&command, cancelled, timeout)
        }
        #[cfg(not(windows))]
        {
            let mut command = command;
            command
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
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
                if cancelled.load(Ordering::Relaxed) || start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = out.join();
                    let _ = err.join();
                    return Err("操作已取消或超时".to_owned());
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
}

#[cfg(any(windows, feature = "windows-check"))]
#[derive(Clone)]
pub enum Backend {
    Windows,
    Paddle(PathBuf),
}
#[cfg(any(windows, feature = "windows-check"))]
impl Backend {
    pub fn selected(settings: pokemmo_hint::engines::EngineSettings) -> Result<Self, String> {
        use pokemmo_hint::engines::{assets_dir, Engine};
        if settings.engine == Engine::Windows {
            return Ok(Self::Windows);
        }
        let dir = assets_dir()?;
        if !pokemmo_hint::engines::cached_assets_ready(&dir) {
            return Err("请打开设置，选择 PP-OCR 并完成下载和检测。".to_owned());
        }
        Ok(Self::Paddle(dir))
    }
    pub fn recognize(
        &self,
        image: &Path,
        alternate: Option<&Path>,
        script: &Path,
        cancelled: &AtomicBool,
    ) -> Result<String, String> {
        match self {
            Self::Windows => OcrProcess::recognize_windows(image, alternate, script, cancelled)
                .map_err(|e| {
                    if e.contains("WINDOWS_CHINESE_OCR_UNAVAILABLE") {
                        "系统中文 OCR 未安装。请打开设置安装，或选择 PP-OCRv6 tiny。".to_owned()
                    } else {
                        e
                    }
                }),
            Self::Paddle(dir) => {
                let mut cmd = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
                cmd.arg("--paddle-image").arg(image).arg(dir);
                if let Some(alt) = alternate {
                    cmd.arg(alt);
                }
                OcrProcess::capture_command(cmd, cancelled)
            }
        }
    }
}
#[cfg(any(windows, feature = "windows-check"))]
pub fn powershell(script: &Path) -> Result<Command, String> {
    let shell = PathBuf::from(std::env::var_os("SystemRoot").ok_or("找不到 Windows 目录")?)
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut cmd = Command::new(shell);
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
    ])
    .arg(script);
    Ok(cmd)
}
