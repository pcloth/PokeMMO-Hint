//! Modal engine and language selection; preparation runs off the UI thread.
use super::*;
use crate::ocr::{powershell, OcrProcess};
use pokemmo_hint::engines::{self, DownloadSource, Engine, EngineSettings};
#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: Handle,
        verb: *const u16,
        file: *const u16,
        args: *const u16,
        dir: *const u16,
        show: i32,
    ) -> isize;
}
struct State {
    value: EngineSettings,
    status: String,
    ready: bool,
    accepted: bool,
    closed: bool,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    rx: Option<mpsc::Receiver<Result<String, String>>>,
}
struct Context {
    state: RefCell<State>,
    tips: RefCell<tooltips::Tooltips>,
    theme: Rc<paint::Theme>,
    dpi: u32,
    text: &'static Strings,
}
impl State {
    fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(job) = self.worker.take() {
            let _ = job.join();
        }
        self.rx = None;
    }
    unsafe fn prepare(&mut self, hwnd: Handle, text: &'static Strings) {
        self.stop();
        self.stop = Arc::new(AtomicBool::new(false));
        self.ready = false;
        self.status = text.settings_preparing.to_owned();
        EnableWindow(GetDlgItem(hwnd, 201), 0);
        let (value, stop) = (self.value, self.stop.clone());
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.worker = Some(thread::spawn(move || {
            let result = prepare(value, text, &stop, &tx);
            let _ = tx.send(result);
        }));
        InvalidateRect(hwnd, null(), 0);
    }
}
fn prepare(
    value: EngineSettings,
    text: &'static Strings,
    stop: &AtomicBool,
    tx: &mpsc::Sender<Result<String, String>>,
) -> Result<String, String> {
    let scratch = Scratch::create()?;
    let language = value.game_language.native_name();
    if value.engine == Engine::Windows {
        let mut cmd = powershell(&scratch.windows_script)?;
        cmd.arg("-CheckOnly")
            .arg("-Language")
            .arg(value.game_language.windows_tag());
        let output = OcrProcess::capture_command(cmd, stop).map_err(|error| {
            if error.contains("WINDOWS_OCR_UNAVAILABLE") {
                i18n::fill(text.settings_windows_missing, &[("lang", language)])
            } else {
                i18n::fill(text.settings_failed, &[("error", &error)])
            }
        })?;
        if !output.contains("WINDOWS_OCR_READY") {
            return Err(text.settings_windows_check_failed.to_owned());
        }
        return Ok(i18n::fill(
            text.settings_windows_ready,
            &[("lang", language)],
        ));
    }
    let cache = engines::assets_dir()?;
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let using_cache = engines::cached_assets_ready(&cache);
    if using_cache {
        let _ = tx.send(Ok("PROGRESS:CACHE loading saved files".to_owned()));
    } else {
        let script = scratch.directory.join("prepare.ps1");
        let progress = scratch.directory.join("progress.txt");
        std::fs::write(&script, include_str!("../prepare_ocr.ps1")).map_err(|e| e.to_string())?;
        let mut cmd = powershell(&script)?;
        cmd.arg("-CacheDir")
            .arg(&cache)
            .arg("-Source")
            .arg(if value.source == DownloadSource::Mirror {
                "mirror"
            } else {
                "official"
            })
            .arg("-ProgressPath")
            .arg(&progress);
        // Progress polling has its own bounded lifetime; the collector owns and cancels the child.
        let done = AtomicBool::new(false);
        thread::scope(|scope| -> Result<(), String> {
            let done_ref = &done;
            let progress_ref = &progress;
            scope.spawn(move || {
                while !done_ref.load(Ordering::Relaxed) {
                    if let Ok(s) = std::fs::read_to_string(progress_ref) {
                        let _ = tx.send(Ok(format!("PROGRESS:{s}")));
                    }
                    thread::sleep(Duration::from_millis(250));
                }
            });
            let result = OcrProcess::capture_timeout(cmd, stop, Duration::from_secs(600));
            done.store(true, Ordering::Relaxed);
            result.map(|_| ())
        })?;
    }
    if stop.load(Ordering::Relaxed) {
        return Err(text.settings_cancelled.to_owned());
    }
    let _ = tx.send(Ok("PROGRESS:CHECK loading model".to_owned()));
    let mut cmd = std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    cmd.arg("--paddle-check").arg(&cache);
    let output =
        OcrProcess::capture_timeout(cmd, stop, Duration::from_secs(60)).map_err(|error| {
            if using_cache && !stop.load(Ordering::Relaxed) {
                // A broken cached runtime must not cause a silent download on opening
                // settings. A deliberate retry will check hashes and repair bad files.
                let _ = std::fs::remove_file(cache.join("ready.txt"));
                i18n::fill(text.settings_cache_failed, &[("error", &error)])
            } else {
                error
            }
        })?;
    if !output.contains("PADDLE_READY") {
        return Err(text.settings_check_failed.to_owned());
    }
    std::fs::write(cache.join("ready.txt"), engines::MODEL_SHA256).map_err(|e| e.to_string())?;
    let mut ready = if using_cache {
        text.settings_paddle_ready_cache.to_owned()
    } else {
        text.settings_paddle_ready_new.to_owned()
    };
    if !value.supports_game_language(value.game_language) {
        ready.push_str("\r\n");
        ready.push_str(text.settings_ppocr_ja_warning);
    }
    Ok(ready)
}
fn progress_text(s: &str, text: &'static Strings) -> String {
    s.replace("PROGRESS:", "")
        .replace("MODEL", text.settings_progress_model)
        .replace("RUNTIME", text.settings_progress_runtime)
        .replace("connecting", text.settings_progress_connecting)
        .replace("extracting", text.settings_progress_extracting)
        .replace(
            "CACHE loading saved files",
            text.settings_progress_cache_loading,
        )
        .replace(
            "CACHE model verified",
            text.settings_progress_cache_model,
        )
        .replace(
            "CACHE runtime verified",
            text.settings_progress_cache_runtime,
        )
        .replace("CHECK loading model", text.settings_progress_check)
}
pub(super) unsafe extern "system" fn window_proc(
    hwnd: Handle,
    msg: u32,
    w: usize,
    l: isize,
) -> isize {
    let p = GetWindowLongPtrW(hwnd, USER_DATA) as *const Context;
    if p.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let ctx = &*p;
    if msg == 0x14 {
        return 1;
    }
    if msg == 0x2b {
        paint::button(&*(l as *const paint::DrawItem), &ctx.theme, ctx.dpi);
        return 1;
    }
    if msg == 0x133 || msg == 0x138 {
        SetBkColor(w as Handle, 0xffffff);
        SetTextColor(w as Handle, 0x3b291e);
        return ctx.theme.white_brush();
    }
    if msg == WM_PAINT {
        let mut ps = Paint::default();
        let dc = BeginPaint(hwnd, &mut ps);
        let mut r = Rect::default();
        GetClientRect(hwnd, &mut r);
        FillRect(dc, &r, ctx.theme.white_brush());
        let d = ctx.dpi;
        let t = ctx.text;
        for (label, x, y) in [
            (t.settings_ui_language, 20, 12),
            (t.settings_game_language, 268, 12),
            (t.settings_engine, 20, 72),
            (t.settings_source, 20, 132),
            (t.settings_refresh, 20, 192),
        ] {
            paint::text(
                dc,
                ctx.theme.body,
                0x3b291e,
                label,
                paint::rect(d, x, y, 240, 24),
                0x20,
            );
        }
        paint::text(
            dc,
            ctx.theme.small,
            0x3b291e,
            t.settings_help,
            paint::rect(d, 20, 258, 500, 96),
            0x10,
        );
        if let Ok(state) = ctx.state.try_borrow() {
            paint::text(
                dc,
                ctx.theme.small,
                if state.ready { 0x698405 } else { 0x8b6540 },
                &state.status,
                paint::rect(d, 20, 360, 500, 60),
                0x10,
            );
        }
        EndPaint(hwnd, &ps);
        return 0;
    }
    if msg == WM_TIMER {
        if let Ok(mut tips) = ctx.tips.try_borrow_mut() {
            tips.tick();
        }
    }
    if msg == WM_DESTROY {
        if let Ok(mut state) = ctx.state.try_borrow_mut() {
            state.closed = true;
        }
        return 0; // Only the main window owns WM_QUIT.
    }
    // DefWindowProcW handles WM_SYSCOMMAND/SC_CLOSE by synchronously sending
    // WM_CLOSE. Release all state borrows before entering that native path.
    if !matches!(msg, WM_COMMAND | WM_TIMER | 0x10) {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let Ok(mut state) = ctx.state.try_borrow_mut() else {
        return DefWindowProcW(hwnd, msg, w, l);
    };
    match msg {
        WM_COMMAND => {
            let id = w & 0xffff;
            let notification = w >> 16;
            if id == 213 && notification == 1 {
                if let Some(lang) = Language::ALL
                    .get(SendMessageW(GetDlgItem(hwnd, 213), 0x147, 0, 0) as usize)
                {
                    state.value.ui_language = *lang;
                }
                return 0; // The dialog keeps its current text until Apply.
            }
            if id == 214 && notification == 1 {
                if let Some(lang) = Language::ALL
                    .get(SendMessageW(GetDlgItem(hwnd, 214), 0x147, 0, 0) as usize)
                {
                    state.value.game_language = *lang;
                }
                state.prepare(hwnd, ctx.text);
                return 0;
            }
            if id == 212 && notification == 1 {
                let index = SendMessageW(GetDlgItem(hwnd, 212), 0x147, 0, 0);
                if let Some(ms) = engines::REFRESH_INTERVALS_MS.get(index as usize) {
                    state.value.refresh_interval_ms = *ms;
                }
                return 0; // Changing refresh rate does not reload or download the engine.
            }
            if (id == 210 || id == 211) && notification == 1 {
                state.value.engine = if SendMessageW(GetDlgItem(hwnd, 210), 0x147, 0, 0) == 1 {
                    Engine::PaddleTiny
                } else {
                    Engine::Windows
                };
                state.value.source = if SendMessageW(GetDlgItem(hwnd, 211), 0x147, 0, 0) == 1 {
                    DownloadSource::Official
                } else {
                    DownloadSource::Mirror
                };
                EnableWindow(
                    GetDlgItem(hwnd, 211),
                    i32::from(state.value.engine == Engine::PaddleTiny),
                );
                state.prepare(hwnd, ctx.text);
                return 0;
            }
            if notification == 0 {
                match id {
                    201 if state.ready => {
                        state.accepted = true;
                        state.closed = true;
                        ShowWindow(hwnd, 0);
                    }
                    202 => {
                        state.closed = true;
                        ShowWindow(hwnd, 0);
                    }
                    203 => state.prepare(hwnd, ctx.text),
                    204 => {
                        let file = if state.value.engine == Engine::Windows {
                            "ms-settings:regionlanguage".to_owned()
                        } else {
                            engines::assets_dir()
                                .map(|p| p.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        };
                        if state.value.engine == Engine::Windows {
                            let language = state.value.game_language.native_name();
                            MessageBoxW(
                                hwnd,
                                wide(&i18n::fill(
                                    ctx.text.settings_install_body,
                                    &[("lang", language)],
                                ))
                                .as_ptr(),
                                wide(&i18n::fill(
                                    ctx.text.settings_install_title,
                                    &[("lang", language)],
                                ))
                                .as_ptr(),
                                0x40,
                            );
                        }
                        ShellExecuteW(
                            hwnd,
                            wide("open").as_ptr(),
                            wide(&file).as_ptr(),
                            null(),
                            null(),
                            1,
                        );
                    }
                    _ => {}
                }
            }
            0
        }
        WM_TIMER => {
            let updates = state
                .rx
                .as_ref()
                .map(|rx| rx.try_iter().collect::<Vec<_>>())
                .unwrap_or_default();
            for update in updates {
                match update {
                    Ok(s) if s.starts_with("PROGRESS:") => {
                        state.status = progress_text(&s, ctx.text)
                    }
                    Ok(s) => {
                        state.ready = true;
                        state.status = s;
                        EnableWindow(GetDlgItem(hwnd, 201), 1);
                    }
                    Err(e) => {
                        state.ready = false;
                        state.status = i18n::fill(ctx.text.settings_failed, &[("error", &e)]);
                    }
                }
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        0x10 => {
            state.closed = true;
            ShowWindow(hwnd, 0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
pub(super) unsafe fn edit(
    parent: Handle,
    value: EngineSettings,
    theme: Rc<paint::Theme>,
    dpi: u32,
    language: Language,
) -> Result<Option<EngineSettings>, String> {
    let text = i18n::strings(language);
    let hwnd = CreateWindowExW(
        0,
        wide("PokeMMOHintSettings").as_ptr(),
        wide(text.settings_title).as_ptr(),
        0x00c80000,
        200,
        160,
        paint::px(550, dpi),
        paint::px(482, dpi),
        parent,
        0,
        GetModuleHandleW(null()),
        null(),
    );
    if hwnd == 0 {
        return Err(text.settings_open_failed.to_owned());
    }
    let mut tips = tooltips::Tooltips::new(hwnd, theme.clone(), dpi);
    for (id, label, x) in [
        (201, text.settings_apply, 20),
        (203, text.settings_retry, 74),
        (204, text.settings_system_language, 128),
        (202, text.settings_cancel, 478),
    ] {
        let b = control(
            hwnd,
            "BUTTON",
            label,
            0,
            id,
            paint::px(x, dpi),
            paint::px(430, dpi),
            paint::px(40, dpi),
            paint::px(36, dpi),
        );
        tips.add(b);
    }
    for (id, y, x, width, labels, index) in [
        (
            213,
            38,
            20,
            240,
            Language::ALL
                .iter()
                .map(|lang| lang.native_name())
                .collect::<Vec<_>>(),
            value.ui_language.index(),
        ),
        (
            214,
            38,
            268,
            250,
            Language::ALL
                .iter()
                .map(|lang| lang.native_name())
                .collect::<Vec<_>>(),
            value.game_language.index(),
        ),
        (
            210,
            98,
            20,
            498,
            vec![text.settings_engine_windows, text.settings_engine_paddle],
            usize::from(value.engine == Engine::PaddleTiny),
        ),
        (
            211,
            158,
            20,
            498,
            vec![text.settings_source_mirror, text.settings_source_official],
            usize::from(value.source == DownloadSource::Official),
        ),
        (
            212,
            218,
            20,
            498,
            text.settings_refresh_options.to_vec(),
            engines::REFRESH_INTERVALS_MS
                .iter()
                .position(|ms| *ms == value.refresh_interval_ms)
                .unwrap_or(1),
        ),
    ] {
        let combo = control(
            hwnd,
            "COMBOBOX",
            "",
            0x00210003,
            id,
            paint::px(x, dpi),
            paint::px(y, dpi),
            paint::px(width, dpi),
            paint::px(200, dpi),
        );
        for label in labels {
            SendMessageW(combo, 0x143, 0, wide(label).as_ptr() as isize);
        }
        SendMessageW(combo, 0x14e, index, 0);
        SendMessageW(combo, 0x30, theme.body as usize, 1);
    }
    EnableWindow(
        GetDlgItem(hwnd, 211),
        i32::from(value.engine == Engine::PaddleTiny),
    );
    let context = Box::new(Context {
        state: RefCell::new(State {
            value,
            status: String::new(),
            ready: false,
            accepted: false,
            closed: false,
            stop: Arc::new(AtomicBool::new(false)),
            worker: None,
            rx: None,
        }),
        tips: RefCell::new(tips),
        theme,
        dpi,
        text,
    });
    SetWindowLongPtrW(hwnd, USER_DATA, (&*context as *const Context) as isize);
    EnableWindow(parent, 0);
    SetTimer(hwnd, 1, 150, null());
    ShowWindow(hwnd, 5);
    context.state.borrow_mut().prepare(hwnd, text);
    let mut msg = Msg::default();
    let mut quit = None;
    loop {
        let closed = context.state.borrow().closed;
        if closed || IsWindow(hwnd) == 0 {
            break;
        }
        let result = GetMessageW(&mut msg, 0, 0, 0);
        if result <= 0 {
            quit = Some(msg.wparam);
            break;
        }
        if msg.message == 0x100 && msg.wparam == 27 {
            context.state.borrow_mut().closed = true;
            continue;
        }
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    // Re-enable the owner before destroying its active modal window, so Windows
    // can restore activation to the main window during destruction.
    EnableWindow(parent, 1);
    if IsWindow(hwnd) != 0 {
        KillTimer(hwnd, 1);
        SetWindowLongPtrW(hwnd, USER_DATA, 0);
        DestroyWindow(hwnd);
    }
    SetForegroundWindow(parent);
    context.state.borrow_mut().stop();
    if let Some(code) = quit {
        PostQuitMessage(code as i32);
    }
    let state = context.state.borrow();
    Ok(state.accepted.then_some(state.value))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Exercise the real Win32 reentrant message path used by the title-bar X
    // and Alt+F4. A direct WM_CLOSE call would miss the original borrow bug.
    #[test]
    fn system_close_exits_the_settings_modal_loop() {
        unsafe {
            let text = i18n::strings(Language::Zh);
            register("PokeMMOHintSettingsCloseTest", window_proc, text).unwrap();
            register("PokeMMOHintButtonTip", tooltips::window_proc, text).unwrap();
            let hwnd = CreateWindowExW(
                0,
                wide("PokeMMOHintSettingsCloseTest").as_ptr(),
                wide("test").as_ptr(),
                0x00c80000,
                0,
                0,
                200,
                200,
                0,
                0,
                GetModuleHandleW(null()),
                null(),
            );
            assert_ne!(hwnd, 0);
            let theme = Rc::new(paint::Theme::new(96));
            let context = Box::new(Context {
                state: RefCell::new(State {
                    value: EngineSettings::default(),
                    status: String::new(),
                    ready: false,
                    accepted: false,
                    closed: false,
                    stop: Arc::new(AtomicBool::new(false)),
                    worker: None,
                    rx: None,
                }),
                tips: RefCell::new(tooltips::Tooltips::new(hwnd, theme.clone(), 96)),
                theme,
                dpi: 96,
                text,
            });
            SetWindowLongPtrW(hwnd, USER_DATA, (&*context as *const Context) as isize);
            ShowWindow(hwnd, 5);
            SendMessageW(hwnd, 0x112, 0xf060, 0); // WM_SYSCOMMAND / SC_CLOSE
            let closed = context.state.borrow().closed;
            let accepted = context.state.borrow().accepted;
            if IsWindow(hwnd) != 0 {
                SetWindowLongPtrW(hwnd, USER_DATA, 0);
                DestroyWindow(hwnd);
            }
            assert!(
                closed,
                "system close must end the modal loop and restore the owner"
            );
            assert!(!accepted, "title-bar close cancels changes");
        }
    }
}
