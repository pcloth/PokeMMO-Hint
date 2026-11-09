//! Native Win32 adapter. No injection, accessibility service, hooks, or game-file writes.
#![allow(non_snake_case, dead_code)]
#[cfg(all(windows, not(target_pointer_width = "64")))]
compile_error!("This prototype requires 64-bit Windows Rust.");
use crate::ocr::Backend;
use pokemmo_hint::bitmap::{binary_bitmap, ocr_bitmap, white_text_bitmap};
use pokemmo_hint::engines::{self, EngineSettings};
use pokemmo_hint::history::History;
use pokemmo_hint::{
    hint_card, lookup_query, resolve_reads, species_data, HintCard, MatchResult, NameSuggestion,
    Roi, Tracker,
};
use std::collections::HashMap;
use std::sync::Mutex;
mod overlay;
mod paint;
mod selection_edit;
mod settings;
mod tooltips;
use pokemmo_hint::regions::{Bounds, RegionSettings, Selection, MAX_REGIONS};
use std::cell::RefCell;
use std::rc::Rc;
use std::{
    ffi::c_void,
    path::PathBuf,
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

type Handle = isize;
type Proc = unsafe extern "system" fn(Handle, u32, usize, isize) -> isize;
#[repr(C)]
#[derive(Default, Copy, Clone)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
#[derive(Default, Copy, Clone)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
#[derive(Default)]
struct Msg {
    hwnd: Handle,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    point: Point,
    private: u32,
}
#[repr(C)]
struct WndClass {
    style: u32,
    proc: Option<Proc>,
    cls_extra: i32,
    wnd_extra: i32,
    instance: Handle,
    icon: Handle,
    cursor: Handle,
    brush: Handle,
    menu: *const u16,
    class: *const u16,
}
#[repr(C)]
#[derive(Default)]
struct Paint {
    dc: Handle,
    erase: i32,
    rect: Rect,
    restore: i32,
    update: i32,
    reserved: [u8; 32],
}
#[repr(C)]
#[derive(Default)]
struct BitmapHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bits: u16,
    compression: u32,
    image_size: u32,
    xppm: i32,
    yppm: i32,
    used: u32,
    important: u32,
}
#[repr(C)]
struct BitmapInfo {
    header: BitmapHeader,
    colors: [u32; 1],
}

#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(hwnd: Handle, text: *const u16, title: *const u16, kind: u32) -> i32;
    fn SetProcessDpiAwarenessContext(context: isize) -> i32;
    fn GetDpiForWindow(hwnd: Handle) -> u32;
    fn AdjustWindowRectExForDpi(rect: *mut Rect, style: u32, menu: i32, ex: u32, dpi: u32) -> i32;
    fn EnableWindow(hwnd: Handle, enable: i32) -> i32;
    fn SetForegroundWindow(hwnd: Handle) -> i32;
    fn GetCursorPos(point: *mut Point) -> i32;
    fn SetWindowDisplayAffinity(hwnd: Handle, affinity: u32) -> i32;
    fn MonitorFromPoint(point: Point, flags: u32) -> Handle;
    fn GetMonitorInfoW(monitor: Handle, info: *mut MonitorInfo) -> i32;
    fn RegisterClassW(class: *const WndClass) -> u16;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: Handle,
        menu: Handle,
        instance: Handle,
        param: *const c_void,
    ) -> Handle;
    fn DefWindowProcW(hwnd: Handle, msg: u32, w: usize, l: isize) -> isize;
    fn GetWindowLongPtrW(hwnd: Handle, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: Handle, index: i32, value: isize) -> isize;
    fn ShowWindow(hwnd: Handle, command: i32) -> i32;
    fn DestroyWindow(hwnd: Handle) -> i32;
    fn GetMessageW(msg: *mut Msg, hwnd: Handle, min: u32, max: u32) -> i32;
    fn TranslateMessage(msg: *const Msg) -> i32;
    fn DispatchMessageW(msg: *const Msg) -> isize;
    fn PostQuitMessage(code: i32);
    fn SetTimer(hwnd: Handle, id: usize, interval: u32, callback: *const c_void) -> usize;
    fn KillTimer(hwnd: Handle, id: usize) -> i32;
    fn SetWindowTextW(hwnd: Handle, text: *const u16) -> i32;
    fn GetWindowTextW(hwnd: Handle, text: *mut u16, max: i32) -> i32;
    fn SendMessageW(hwnd: Handle, msg: u32, w: usize, l: isize) -> isize;
    fn IsWindowVisible(hwnd: Handle) -> i32;
    fn IsIconic(hwnd: Handle) -> i32;
    fn IsWindow(hwnd: Handle) -> i32;
    fn GetClientRect(hwnd: Handle, rect: *mut Rect) -> i32;
    fn GetWindowRect(hwnd: Handle, rect: *mut Rect) -> i32;
    fn ScreenToClient(hwnd: Handle, point: *mut Point) -> i32;
    fn GetForegroundWindow() -> Handle;
    fn SetWindowPos(hwnd: Handle, after: Handle, x: i32, y: i32, w: i32, h: i32, flags: u32)
        -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
    fn SetLayeredWindowAttributes(hwnd: Handle, key: u32, alpha: u8, flags: u32) -> i32;
    fn SetCapture(hwnd: Handle) -> Handle;
    fn ReleaseCapture() -> i32;
    fn InvalidateRect(hwnd: Handle, rect: *const Rect, erase: i32) -> i32;
    fn BeginPaint(hwnd: Handle, paint: *mut Paint) -> Handle;
    fn EndPaint(hwnd: Handle, paint: *const Paint) -> i32;
    fn FillRect(dc: Handle, rect: *const Rect, brush: Handle) -> i32;
    fn DrawTextW(dc: Handle, text: *const u16, len: i32, rect: *mut Rect, format: u32) -> i32;
    fn LoadCursorW(instance: Handle, name: *const u16) -> Handle;
    fn SetCursor(cursor: Handle) -> Handle;
    fn GetDlgItem(hwnd: Handle, id: i32) -> Handle;
    fn DrawIconEx(
        dc: Handle,
        x: i32,
        y: i32,
        icon: Handle,
        width: i32,
        height: i32,
        step: u32,
        brush: Handle,
        flags: u32,
    ) -> i32;
    fn DrawFocusRect(dc: Handle, rect: *const Rect) -> i32;
    fn LoadImageW(
        instance: Handle,
        name: *const u16,
        kind: u32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> Handle;
    fn GetDC(hwnd: Handle) -> Handle;
    fn ReleaseDC(hwnd: Handle, dc: Handle) -> i32;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Handle;
    fn GetLocalTime(time: *mut SystemTimeFields);
}
#[link(name = "gdi32")]
extern "system" {
    fn GetStockObject(index: i32) -> Handle;
    fn CreateCompatibleDC(dc: Handle) -> Handle;
    fn DeleteDC(dc: Handle) -> i32;
    fn CreateDIBSection(
        dc: Handle,
        info: *const BitmapInfo,
        usage: u32,
        bits: *mut *mut c_void,
        section: Handle,
        offset: u32,
    ) -> Handle;
    fn SelectObject(dc: Handle, obj: Handle) -> Handle;
    fn DeleteObject(obj: Handle) -> i32;
    fn BitBlt(
        dest: Handle,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        src: Handle,
        sx: i32,
        sy: i32,
        mode: u32,
    ) -> i32;
    fn CreatePen(style: i32, width: i32, color: u32) -> Handle;
    fn Rectangle(dc: Handle, left: i32, top: i32, right: i32, bottom: i32) -> i32;
    fn SetBkMode(dc: Handle, mode: i32) -> i32;
    fn SetTextColor(dc: Handle, color: u32) -> u32;
    fn SetBkColor(dc: Handle, color: u32) -> u32;
    fn CreateSolidBrush(color: u32) -> Handle;
    fn CreateRoundRectRgn(
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        width: i32,
        height: i32,
    ) -> Handle;
    fn CreatePolygonRgn(points: *const Point, count: i32, mode: i32) -> Handle;
    fn CombineRgn(destination: Handle, first: Handle, second: Handle, mode: i32) -> i32;
    fn FillRgn(dc: Handle, region: Handle, brush: Handle) -> i32;
    fn FrameRgn(dc: Handle, region: Handle, brush: Handle, width: i32, height: i32) -> i32;
    fn RoundRect(dc: Handle, left: i32, top: i32, right: i32, bottom: i32, w: i32, h: i32) -> i32;
    fn CreateFontW(
        height: i32,
        width: i32,
        escape: i32,
        orientation: i32,
        weight: i32,
        italic: u32,
        underline: u32,
        strike: u32,
        charset: u32,
        out: u32,
        clip: u32,
        quality: u32,
        pitch: u32,
        face: *const u16,
    ) -> Handle;
    fn GetTextExtentPoint32W(dc: Handle, text: *const u16, len: i32, size: *mut Point) -> i32;
    fn CreateCompatibleBitmap(dc: Handle, width: i32, height: i32) -> Handle;
    fn GdiFlush() -> i32;
    fn SaveDC(dc: Handle) -> i32;
    fn RestoreDC(dc: Handle, saved: i32) -> i32;
    fn SetViewportOrgEx(dc: Handle, x: i32, y: i32, old: *mut Point) -> i32;
}
#[repr(C)]
struct MonitorInfo {
    size: u32,
    monitor: Rect,
    work: Rect,
    flags: u32,
}
const WM_DESTROY: u32 = 2;
const WM_PAINT: u32 = 15;
const WM_COMMAND: u32 = 0x111;
const WM_TIMER: u32 = 0x113;
const USER_DATA: i32 = -21;
// Right-hand recognition history column, in logical pixels.
const HISTORY_LIST_ID: isize = 112;
const HISTORY_LEFT: i32 = 620;
const HISTORY_TOP: i32 = 58;
const HISTORY_WIDTH: i32 = 240;
const HISTORY_HEIGHT: i32 = 408;
const HISTORY_LIST_LEFT: i32 = 628;
const HISTORY_LIST_TOP: i32 = 96;
const HISTORY_LIST_WIDTH: i32 = 224;
const HISTORY_LIST_HEIGHT: i32 = 358;
#[repr(C)]
#[derive(Default)]
struct SystemTimeFields {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16,
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
unsafe fn set_text(hwnd: Handle, text: &str) {
    SetWindowTextW(hwnd, wide(text).as_ptr());
}
unsafe fn get_text(hwnd: Handle) -> String {
    let mut b = [0u16; 512];
    let n = GetWindowTextW(hwnd, b.as_mut_ptr(), 512);
    String::from_utf16_lossy(&b[..n.max(0) as usize])
}
fn local_clock() -> String {
    let mut time = SystemTimeFields::default();
    unsafe {
        GetLocalTime(&mut time);
    }
    format!(
        "{:02}-{:02} {:02}:{:02}",
        time.month, time.day, time.hour, time.minute
    )
}

unsafe fn desktop_bounds() -> Bounds {
    Bounds {
        x: GetSystemMetrics(76),
        y: GetSystemMetrics(77),
        width: GetSystemMetrics(78),
        height: GetSystemMetrics(79),
    }
}
unsafe fn screen_rect(selected: Selection) -> Result<Rect, String> {
    selected.screen_bounds(desktop_bounds()).map(overlay::rect)
}
fn intersects(a: Rect, b: Rect) -> bool {
    a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top
}

#[derive(Clone)]
struct Update {
    index: usize,
    deferred: bool,
    status: String,
    body: String,
    card: Option<HintCard>,
    suggestion: Option<NameSuggestion>,
}
struct SlotState {
    last: Option<Update>,
    updated: Instant,
    tag: Option<overlay::Window>,
}
impl Default for SlotState {
    fn default() -> Self {
        Self {
            last: None,
            updated: Instant::now(),
            tag: None,
        }
    }
}
struct App {
    hwnd: Handle,
    status: String,
    body: String,
    card: Option<HintCard>,
    card_height: i32,
    suggestion: Option<NameSuggestion>,
    confirm: Handle,
    theme: Rc<paint::Theme>,
    dpi: u32,
    confirmed: Arc<Mutex<HashMap<String, u16>>>,
    edit: Handle,
    start: Handle,
    settings: RegionSettings,
    engine_settings: EngineSettings,
    selector: Handle,
    history: History,
    history_list: Handle,
    slots: Vec<SlotState>,
    details: Option<overlay::Window>,
    hovered: Option<usize>,
    last_hover: Instant,
    occlusions: Arc<Mutex<Vec<Rect>>>,
    manual_preview: bool,
    rx: Option<mpsc::Receiver<Update>>,
    stop: Arc<AtomicBool>,
    workers: Vec<thread::JoinHandle<()>>,
}
struct UiContext {
    app: RefCell<App>,
    tooltips: RefCell<tooltips::Tooltips>,
    theme: Rc<paint::Theme>,
    dpi: u32,
}
impl App {
    fn halt(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
        self.rx = None;
        self.slots.clear();
        self.details = None;
        self.hovered = None;
        if let Ok(mut bounds) = self.occlusions.lock() {
            bounds.clear();
        }
        unsafe {
            set_text(self.start, "开始识别");
            InvalidateRect(self.hwnd, null(), 0);
        }
    }
    fn status(&mut self, text: &str) {
        self.status = text.to_owned();
        unsafe {
            InvalidateRect(self.hwnd, null(), 0);
        }
    }
    fn content(&mut self, body: &str, card: Option<HintCard>, suggestion: Option<NameSuggestion>) {
        self.body = body.to_owned();
        self.card = card;
        self.suggestion = suggestion;
        self.layout_card();
        unsafe {
            if let Some(s) = &self.suggestion {
                if let Some(species) = species_data().iter().find(|p| p.id == s.id) {
                    set_text(self.confirm, &format!("确认为 {}", species.name));
                    ShowWindow(self.confirm, 5);
                }
            } else {
                ShowWindow(self.confirm, 0);
            }
            InvalidateRect(self.hwnd, null(), 0);
        }
    }
    fn layout_card(&mut self) {
        unsafe {
            if IsIconic(self.hwnd) != 0 {
                return;
            }
            let height = self
                .card
                .as_ref()
                .map(|card| paint::card_height(&self.theme, self.dpi, card))
                .unwrap_or(258)
                .max(258);
            if height == self.card_height {
                return;
            }
            self.card_height = height;
            let offset = height - 258;
            for (control, x, y, w, h) in [
                (self.edit, 20, 436, 170, 32),
                (GetDlgItem(self.hwnd, 104), 200, 435, 44, 34),
                (self.confirm, 254, 435, 44, 34),
            ] {
                SetWindowPos(
                    control,
                    0,
                    paint::px(x, self.dpi),
                    paint::px(y + offset, self.dpi),
                    paint::px(w, self.dpi),
                    paint::px(h, self.dpi),
                    0x14,
                );
            }
            let mut bounds = Rect {
                left: 0,
                top: 0,
                right: paint::px(880, self.dpi),
                bottom: paint::px(480 + offset, self.dpi),
            };
            AdjustWindowRectExForDpi(&mut bounds, 0x02ca0000, 0, 0x8, self.dpi);
            SetWindowPos(
                self.hwnd,
                0,
                0,
                0,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                0x16,
            );
            SetWindowPos(
                self.history_list,
                0,
                paint::px(HISTORY_LIST_LEFT, self.dpi),
                paint::px(HISTORY_LIST_TOP, self.dpi),
                paint::px(HISTORY_LIST_WIDTH, self.dpi),
                paint::px(HISTORY_LIST_HEIGHT + offset, self.dpi),
                0x14,
            );
        }
    }
    fn persist(&mut self) -> Result<(), String> {
        unsafe {
            if IsIconic(self.hwnd) == 0 {
                let mut r = Rect::default();
                if GetWindowRect(self.hwnd, &mut r) != 0 {
                    self.settings.window = Some((r.left, r.top));
                }
            }
        }
        save_settings(&self.settings)
    }
    fn rebuild_selector(&self) {
        unsafe {
            SendMessageW(self.selector, 0x14b, 0, 0);
            for i in 0..self.settings.selections.len() {
                SendMessageW(
                    self.selector,
                    0x143,
                    0,
                    wide(&self.settings.name(i)).as_ptr() as isize,
                );
            }
            if !self.settings.selections.is_empty() {
                SendMessageW(self.selector, 0x14e, self.settings.selected, 0);
            }
        }
    }
    fn preview(&mut self) {
        if self.manual_preview {
            return;
        }
        let index = self.settings.selected;
        if let Some(update) = self.slots.get(index).and_then(|s| s.last.clone()) {
            self.status(&format!(
                "{} · {}",
                self.settings.name(index),
                update.status
            ));
            self.content(&update.body, update.card, update.suggestion);
        } else {
            self.status(&format!(
                "已保存 {} 个选区；当前查看选区 {}",
                self.settings.selections.len(),
                index + 1
            ));
            self.content("开始识别后，每个选区会独立显示弱点标签。", None, None);
        }
    }
    fn refresh_history(&mut self) {
        unsafe {
            SendMessageW(self.history_list, 0x184, 0, 0); // LB_RESETCONTENT
            let data = species_data();
            for entry in self.history.entries() {
                let name = data
                    .iter()
                    .find(|species| species.id == entry.id)
                    .map(|species| species.name.as_str())
                    .unwrap_or("未知");
                let label = format!("{}  #{:03} {}", entry.clock, entry.id, name);
                SendMessageW(
                    self.history_list,
                    0x180, // LB_ADDSTRING
                    0,
                    wide(&label).as_ptr() as isize,
                );
            }
            if !self.history.is_empty() {
                SendMessageW(self.history_list, 0x186, usize::MAX, 0); // LB_SETCURSEL
            }
            ShowWindow(self.history_list, if self.history.is_empty() { 0 } else { 5 });
            InvalidateRect(self.hwnd, null(), 0);
        }
    }
    fn clear_history_selection(&mut self) {
        unsafe {
            SendMessageW(self.history_list, 0x186, usize::MAX, 0); // LB_SETCURSEL
        }
    }
    fn view_history(&mut self, index: usize) {
        let Some(entry) = self.history.get(index).cloned() else {
            return;
        };
        let data = species_data();
        let Some(species) = data.iter().find(|species| species.id == entry.id) else {
            return;
        };
        // Keep the stored card on screen instead of the next polling result.
        self.manual_preview = true;
        self.content("", Some(hint_card(species)), None);
        self.status(&format!(
            "历史记录 · #{:03} {} · {}",
            entry.id, species.name, entry.region
        ));
    }
    fn start(&mut self) -> Result<(), String> {
        self.halt();
        self.manual_preview = false;
        if self.settings.selections.is_empty() {
            return Err("请先添加至少一个选区".to_owned());
        }
        if self.settings.selections.len() > MAX_REGIONS {
            return Err(format!("最多支持 {MAX_REGIONS} 个选区"));
        }
        let config = Backend::selected(self.engine_settings)?;
        for s in &self.settings.selections {
            s.roi.validate(s.width, s.height)?;
        }
        self.stop = Arc::new(AtomicBool::new(false));
        self.slots = (0..self.settings.selections.len())
            .map(|_| SlotState::default())
            .collect();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        for group in 0..self.settings.selections.len().min(2) {
            let selections = self
                .settings
                .selections
                .iter()
                .copied()
                .enumerate()
                .skip(group)
                .step_by(2)
                .collect();
            let context = WorkerContext {
                config: config.clone(),
                refresh_interval: Duration::from_millis(self.engine_settings.refresh_interval_ms),
                own: self.hwnd,
                stop: self.stop.clone(),
                tx: tx.clone(),
                confirmed: self.confirmed.clone(),
                occlusions: self.occlusions.clone(),
            };
            self.workers
                .push(thread::spawn(move || watch(context, selections)));
        }
        unsafe {
            set_text(self.start, "暂停识别");
        }
        self.content("正在读取画面，请稍候…", None, None);
        self.status(&format!(
            "持续识别 {} 个选区；可最小化此窗口。",
            self.settings.selections.len()
        ));
        Ok(())
    }
    fn sync_overlays(&mut self) {
        unsafe {
            let active = !self.workers.is_empty();
            if !active {
                for slot in &mut self.slots {
                    if let Some(tag) = &slot.tag {
                        tag.hide();
                    }
                }
                if let Some(details) = &self.details {
                    details.hide();
                }
                self.hovered = None;
                if let Ok(mut bounds) = self.occlusions.lock() {
                    bounds.clear();
                }
                return;
            }
            let mut visible = Vec::new();
            let mut obstacles = Vec::new();
            let mut expired = false;
            for (index, selection) in self.settings.selections.iter().enumerate() {
                let Some(slot) = self.slots.get_mut(index) else {
                    continue;
                };
                if slot.updated.elapsed() > Duration::from_secs(15) && slot.last.take().is_some() {
                    expired = true;
                }
                let region = screen_rect(*selection).ok();
                if let Some(r) = region {
                    obstacles.push(overlay::bounds(r));
                }
                let card = slot.last.as_ref().and_then(|u| u.card.clone());
                if let (Some(card), Some(region)) = (card, region) {
                    if slot.tag.is_none() {
                        slot.tag =
                            overlay::Window::new(self.theme.clone(), self.dpi, &card, false).ok();
                    }
                    if let Some(tag) = &mut slot.tag {
                        tag.set_card(&card);
                        let region = overlay::bounds(region);
                        let screen = overlay::screen_for(region);
                        let position =
                            pokemmo_hint::regions::tag_position(region, tag.size(), screen);
                        if position.overlaps(region) {
                            tag.hide();
                            continue;
                        }
                        tag.show_at(position, region);
                        visible.push((index, card, region, position));
                    }
                } else if let Some(tag) = &slot.tag {
                    tag.hide();
                }
            }
            let mut cursor = Point::default();
            GetCursorPos(&mut cursor);
            if let Some((index, _, _, _)) = visible
                .iter()
                .find(|(_, _, _, tag)| tag.contains(cursor.x, cursor.y))
            {
                self.hovered = Some(*index);
                self.last_hover = Instant::now();
            } else if self
                .details
                .as_ref()
                .and_then(|d| d.visible_bounds())
                .is_some_and(|r| r.contains(cursor.x, cursor.y))
            {
                self.last_hover = Instant::now();
            } else if self.last_hover.elapsed() > Duration::from_millis(350) {
                self.hovered = None;
            }
            let hovered = self
                .hovered
                .and_then(|index| visible.iter().find(|(i, _, _, _)| *i == index));
            if let Some((_, card, region, _)) = hovered {
                if self.details.is_none() {
                    self.details =
                        overlay::Window::new(self.theme.clone(), self.dpi, card, true).ok();
                }
                if let Some(details) = &mut self.details {
                    details.set_card(card);
                    let mut avoid = obstacles.clone();
                    avoid.extend(visible.iter().map(|(_, _, _, r)| *r));
                    let position = pokemmo_hint::regions::detail_position(
                        *region,
                        details.size(),
                        overlay::screen_for(*region),
                        &avoid,
                    );
                    details.show_at(position, *region);
                }
            } else {
                self.hovered = None;
                if let Some(details) = &self.details {
                    details.hide();
                }
            }
            let mut windows: Vec<Rect> = visible
                .iter()
                .map(|(_, _, _, r)| overlay::rect(*r))
                .collect();
            if let Some(r) = self.details.as_ref().and_then(|d| d.visible_bounds()) {
                windows.push(overlay::rect(r));
            }
            if let Ok(mut bounds) = self.occlusions.lock() {
                *bounds = windows;
            }
            if expired {
                self.preview();
            }
        }
    }
}

unsafe extern "system" fn main_proc(hwnd: Handle, msg: u32, w: usize, l: isize) -> isize {
    if msg == 0x14 {
        return 1;
    } // custom-painted background
    if !matches!(
        msg,
        WM_COMMAND | WM_TIMER | WM_DESTROY | WM_PAINT | 0x2b | 0x133 | 0x134 | 0x232 | 0x5
    ) {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let p = GetWindowLongPtrW(hwnd, USER_DATA) as *const UiContext;
    if p.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    if msg == WM_PAINT {
        let app = (&*p).app.try_borrow().ok();
        paint::paint_window(hwnd, app.as_deref());
        return 0;
    }
    if msg == 0x2b {
        paint::button(&*(l as *const paint::DrawItem), &(*p).theme, (*p).dpi);
        return 1;
    }
    if msg == 0x133 || msg == 0x134 {
        SetBkColor(w as Handle, 0xffffff);
        SetTextColor(w as Handle, 0x3b291e);
        return (*p).theme.white_brush();
    }
    if msg == WM_TIMER {
        if let Ok(mut tips) = (&*p).tooltips.try_borrow_mut() {
            tips.tick();
        }
    }
    // Win32 can re-enter this callback during native calls and the selection loop.
    let Ok(mut app) = (&*p).app.try_borrow_mut() else {
        if msg == WM_DESTROY {
            KillTimer(hwnd, 1);
            PostQuitMessage(0);
        }
        return DefWindowProcW(hwnd, msg, w, l);
    };
    match msg {
        0x5 if w == 0 => {
            app.layout_card();
            0
        } // SIZE_RESTORED: results may arrive while minimized.
        WM_COMMAND => {
            let id = w & 0xffff;
            let notification = w >> 16;
            if id == 108 && notification == 1 {
                let selected = SendMessageW(app.selector, 0x147, 0, 0);
                if selected >= 0 {
                    app.settings.selected = selected as usize;
                    app.manual_preview = false;
                    app.clear_history_selection();
                    let _ = app.persist();
                    app.preview();
                }
                return 0;
            }
            if id == HISTORY_LIST_ID as usize && notification == 1 {
                let selected = SendMessageW(app.history_list, 0x0188, 0, 0); // LB_GETCURSEL
                if selected >= 0 {
                    app.view_history(selected as usize);
                }
                return 0;
            }
            if notification != 0 {
                return DefWindowProcW(hwnd, msg, w, l);
            }
            match id {
                101 => {
                    if app.settings.selections.len() >= MAX_REGIONS {
                        app.status(&format!("最多支持 {MAX_REGIONS} 个选区"));
                        return 0;
                    }
                    let resume = !app.workers.is_empty();
                    app.halt();
                    app.manual_preview = false;
                    match choose_roi(hwnd) {
                        Ok(Some(selection)) => {
                            app.settings.selections.push(selection);
                            let count = app.settings.selections.len();
                            app.settings.names.resize(count, String::new());
                            app.settings.selected = app.settings.selections.len() - 1;
                            app.rebuild_selector();
                            if let Err(e) = app.persist() {
                                app.status(&format!("选区已添加，但保存失败：{e}"));
                            } else {
                                app.preview();
                            }
                        }
                        Ok(None) => app.status("已取消添加，原选区保留。"),
                        Err(e) => app.status(&e),
                    }
                    if resume {
                        if let Err(e) = app.start() {
                            app.status(&e);
                        }
                    }
                }
                111 => {
                    let resume = !app.workers.is_empty();
                    app.halt();
                    match settings::edit(hwnd, app.engine_settings, app.theme.clone(), app.dpi) {
                        Ok(Some(value)) => match engines::save_settings(value) {
                            Ok(()) => {
                                app.engine_settings = value;
                                app.status("识别设置已保存。");
                            }
                            Err(e) => app.status(&format!("保存设置失败：{e}")),
                        },
                        Ok(None) => app.status("已取消设置。"),
                        Err(e) => app.status(&e),
                    }
                    if resume {
                        if let Err(e) = app.start() {
                            app.status(&e);
                        }
                    }
                }
                102 => {
                    if !app.workers.is_empty() {
                        app.halt();
                        app.status("已暂停，浮动标签已隐藏。");
                        app.content("可继续添加选区，或使用下方查询。", None, None);
                    } else {
                        app.clear_history_selection();
                        match app.start() {
                            Ok(()) => {
                                ShowWindow(hwnd, 6);
                            } // SW_MINIMIZE, only after Start succeeds
                            Err(e) => app.status(&e),
                        }
                    }
                }
                103 => {
                    let resume = !app.workers.is_empty();
                    app.halt();
                    app.manual_preview = false;
                    if !app.settings.selections.is_empty() {
                        let index = app.settings.selected;
                        app.settings.selections.remove(index);
                        if index < app.settings.names.len() {
                            app.settings.names.remove(index);
                        }
                        app.settings.selected =
                            index.min(app.settings.selections.len().saturating_sub(1));
                    }
                    app.rebuild_selector();
                    if let Err(e) = app.persist() {
                        app.status(&e);
                    } else {
                        app.preview();
                    }
                    if resume && !app.settings.selections.is_empty() {
                        if let Err(e) = app.start() {
                            app.status(&e);
                        }
                    }
                }
                110 => {
                    let index = app.settings.selected;
                    let Some(selected) = app.settings.selections.get(index).copied() else {
                        app.status("请先添加或选择一个选区。");
                        return 0;
                    };
                    let name = app.settings.name(index);
                    let others = app
                        .settings
                        .selections
                        .iter()
                        .enumerate()
                        .filter(|(i, s)| *i != index && s.desktop() == selected.desktop())
                        .map(|(_, s)| s.roi)
                        .collect();
                    let resume = !app.workers.is_empty();
                    app.halt();
                    match selection_edit::edit(hwnd, selected, name, others) {
                        Ok(Some(selection)) => {
                            app.settings.selections[index] = selection;
                            if let Err(e) = app.persist() {
                                app.status(&e);
                            } else {
                                app.status("选区位置和大小已保存。");
                            }
                        }
                        Ok(None) => app.status("已取消修改，保留原选区。"),
                        Err(e) => app.status(&e),
                    }
                    if resume {
                        if let Err(e) = app.start() {
                            app.status(&e);
                        }
                    }
                }
                109 => {
                    let name = get_text(app.selector);
                    let index = app.settings.selected;
                    match app.settings.rename(index, &name) {
                        Ok(()) => {
                            app.rebuild_selector();
                            if let Err(e) = app.persist() {
                                app.status(&e);
                            } else {
                                app.status("选区名称已保存；留空可恢复默认名称。");
                            }
                        }
                        Err(e) => app.status(&e),
                    }
                }
                107 => {
                    app.halt();
                    app.manual_preview = false;
                    app.settings.selections.clear();
                    app.settings.names.clear();
                    app.settings.selected = 0;
                    app.rebuild_selector();
                    app.content("请添加新的识别区域。", None, None);
                    if let Err(e) = app.persist() {
                        app.status(&e);
                    } else {
                        app.status("所有选区已清空并保存。");
                    }
                }
                106 => {
                    if let Some(candidate) = app.suggestion.clone() {
                        if let Ok(mut aliases) = app.confirmed.lock() {
                            aliases.insert(candidate.observed.clone(), candidate.id);
                        }
                        app.content("已确认，下次识别到相同文字时会使用这个物种。", None, None);
                        app.status("已记住本次确认；仅在本次运行中有效。");
                    }
                }
                104 => {
                    app.manual_preview = true;
                    app.clear_history_selection();
                    let data = species_data();
                    match lookup_query(&get_text(app.edit), &data) {
                        MatchResult::Found(id) => {
                            app.content(
                                "",
                                Some(hint_card(data.iter().find(|s| s.id == id).unwrap())),
                                None,
                            );
                            app.status("手动查询结果；选择选区可返回实时信息。");
                        }
                        _ => {
                            app.content(
                                "未找到唯一匹配，请输入中文名或 1–649 全国编号。",
                                None,
                                None,
                            );
                            app.status("手动查询未匹配");
                        }
                    }
                }
                _ => {}
            }
            0
        }
        WM_TIMER => {
            let mut updates = Vec::new();
            if let Some(rx) = &app.rx {
                while let Ok(update) = rx.try_recv() {
                    updates.push(update);
                }
            }
            let mut refresh = false;
            let mut history_changed = false;
            for update in updates {
                if update.deferred {
                    continue;
                }
                let index = update.index;
                let card_id = update.card.as_ref().map(|card| card.id);
                if let Some(slot) = app.slots.get_mut(index) {
                    slot.last = Some(update);
                    slot.updated = Instant::now();
                    refresh |= index == app.settings.selected;
                }
                if let Some(id) = card_id {
                    let region = app.settings.name(index);
                    if app.history.record(id, &region, &local_clock()) {
                        history_changed = true;
                    }
                }
            }
            if history_changed {
                app.refresh_history();
                let _ = save_history(&app.history);
            }
            if refresh {
                app.preview();
            }
            app.sync_overlays();
            0
        }
        0x232 => {
            if let Err(e) = app.persist() {
                app.status(&e);
            }
            0
        }
        WM_DESTROY => {
            let _ = app.persist();
            app.halt();
            KillTimer(hwnd, 1);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}

// Thin wrapper around CreateWindowExW; keep the native coordinates explicit.
#[allow(clippy::too_many_arguments)]
unsafe fn control(
    parent: Handle,
    kind: &str,
    text: &str,
    style: u32,
    id: isize,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Handle {
    let hwnd = CreateWindowExW(
        0,
        wide(kind).as_ptr(),
        wide(text).as_ptr(),
        0x50000000 | style | if kind == "BUTTON" { 0x0001000b } else { 0 },
        x,
        y,
        width,
        height,
        parent,
        id,
        GetModuleHandleW(null()),
        null(),
    );
    if hwnd != 0 {
        SendMessageW(hwnd, 0x30, GetStockObject(17) as usize, 1);
    }
    hwnd
}
// Win32 MAKEINTRESOURCEW uses a low-word resource ID, not a dereferenceable pointer.
fn integer_resource(id: u16) -> *const u16 {
    id as usize as *const u16
}
unsafe fn register(name: &str, proc: Proc) -> Result<(), String> {
    let name = wide(name);
    let c = WndClass {
        style: 3,
        proc: Some(proc),
        cls_extra: 0,
        wnd_extra: 0,
        instance: GetModuleHandleW(null()),
        icon: LoadImageW(
            GetModuleHandleW(null()),
            integer_resource(1),
            1,
            0,
            0,
            0x8040,
        ),
        cursor: LoadCursorW(0, 32512usize as *const u16),
        brush: 6,
        menu: null(),
        class: name.as_ptr(),
    };
    if RegisterClassW(&c) == 0 {
        return Err(format!("注册窗口失败：{}", std::io::Error::last_os_error()));
    }
    Ok(())
}
pub fn show_startup_error(error: &str) {
    unsafe {
        MessageBoxW(
            0,
            wide(error).as_ptr(),
            wide("PokeMMO 属性提示 · 启动失败").as_ptr(),
            0x10,
        );
    }
}
pub fn run() -> Result<(), String> {
    unsafe {
        SetProcessDpiAwarenessContext(-4);
        register("PokeMMOHintWindow", main_proc)?;
        register("PokeMMOHintSelect", select_proc)?;
        register("PokeMMOHintOverlay", overlay::window_proc)?;
        register("PokeMMOHintEditRegion", selection_edit::window_proc)?;
        register("PokeMMOHintButtonTip", tooltips::window_proc)?;
        register("PokeMMOHintSettings", settings::window_proc)?;
        let (settings, load_error) = load_settings();
        let hwnd = CreateWindowExW(
            0x8,
            wide("PokeMMOHintWindow").as_ptr(),
            wide("PokeMMO 属性提示").as_ptr(),
            0x02CA0000,
            100,
            100,
            880,
            310,
            0,
            0,
            GetModuleHandleW(null()),
            null(),
        );
        if hwnd == 0 {
            return Err(format!("创建窗口失败：{}", std::io::Error::last_os_error()));
        }
        let dpi = GetDpiForWindow(hwnd).max(96);
        for (kind, size) in [(0, GetSystemMetrics(49)), (1, GetSystemMetrics(11))] {
            let icon = LoadImageW(
                GetModuleHandleW(null()),
                integer_resource(1),
                1,
                size,
                size,
                0x8000,
            );
            if icon != 0 {
                SendMessageW(hwnd, 0x80, kind, icon);
            }
        }
        let mut bounds = Rect {
            left: 0,
            top: 0,
            right: paint::px(880, dpi),
            bottom: paint::px(480, dpi),
        };
        AdjustWindowRectExForDpi(&mut bounds, 0x02ca0000, 0, 0x8, dpi);
        SetWindowPos(
            hwnd,
            -1,
            0,
            0,
            bounds.right - bounds.left,
            bounds.bottom - bounds.top,
            0x12,
        );
        let mut app = Box::new(App {
            hwnd,
            status: "框选一行名字或 No.编号，然后开始识别。".to_owned(),
            body: "也可以在下方输入中文名字或全国编号查询。".to_owned(),
            card: None,
            card_height: 258,
            suggestion: None,
            confirm: 0,
            theme: Rc::new(paint::Theme::new(dpi)),
            dpi,
            confirmed: Arc::new(Mutex::new(HashMap::new())),
            edit: 0,
            start: 0,
            settings,
            engine_settings: engines::load_settings().unwrap_or_default(),
            selector: 0,
            history: load_history(),
            history_list: 0,
            slots: Vec::new(),
            details: None,
            hovered: None,
            last_hover: Instant::now(),
            occlusions: Arc::new(Mutex::new(Vec::new())),
            manual_preview: false,
            rx: None,
            stop: Arc::new(AtomicBool::new(true)),
            workers: Vec::new(),
        });
        let px = |value| paint::px(value, dpi);
        app.start = control(
            hwnd,
            "BUTTON",
            "开始识别",
            0,
            102,
            px(20),
            px(58),
            px(44),
            px(36),
        );
        control(
            hwnd,
            "BUTTON",
            "添加选区",
            0,
            101,
            px(74),
            px(58),
            px(44),
            px(36),
        );
        control(
            hwnd,
            "BUTTON",
            "删除当前选区",
            0,
            103,
            px(128),
            px(58),
            px(44),
            px(36),
        );
        control(
            hwnd,
            "BUTTON",
            "清空选区",
            0,
            107,
            px(182),
            px(58),
            px(44),
            px(36),
        );
        app.edit = control(
            hwnd,
            "EDIT",
            "",
            0x00810080,
            105,
            px(20),
            px(436),
            px(170),
            px(32),
        );
        SendMessageW(app.edit, 0x30, app.theme.body as usize, 1);
        SendMessageW(app.edit, 0x1501, 0, wide("名字或编号").as_ptr() as isize);
        control(
            hwnd,
            "BUTTON",
            "查询",
            0,
            104,
            px(200),
            px(435),
            px(44),
            px(34),
        );
        app.confirm = control(
            hwnd,
            "BUTTON",
            "确认候选",
            0,
            106,
            px(254),
            px(435),
            px(44),
            px(34),
        );
        ShowWindow(app.confirm, 0);
        app.selector = control(
            hwnd,
            "COMBOBOX",
            "",
            0x00210042,
            108,
            px(20),
            px(101),
            px(200),
            px(250),
        );
        SendMessageW(app.selector, 0x30, app.theme.body as usize, 1);
        SendMessageW(app.selector, 0x141, 32, 0);
        control(
            hwnd,
            "BUTTON",
            "保存名称",
            0,
            109,
            px(266),
            px(100),
            px(36),
            px(30),
        );
        control(
            hwnd,
            "BUTTON",
            "查看并调整当前选区",
            0,
            110,
            px(224),
            px(100),
            px(36),
            px(30),
        );
        let mut tooltips = tooltips::Tooltips::new(hwnd, app.theme.clone(), dpi);
        app.history_list = control(
            hwnd,
            "LISTBOX",
            "",
            0x00210141, // WS_VSCROLL | WS_TABSTOP | LBS_NOINTEGRALHEIGHT | LBS_HASSTRINGS | LBS_NOTIFY
            HISTORY_LIST_ID,
            px(HISTORY_LIST_LEFT),
            px(HISTORY_LIST_TOP),
            px(HISTORY_LIST_WIDTH),
            px(HISTORY_LIST_HEIGHT),
        );
        SendMessageW(app.history_list, 0x30, app.theme.small as usize, 1);
        control(
            hwnd,
            "BUTTON",
            "识别设置",
            0,
            111,
            px(236),
            px(58),
            px(44),
            px(36),
        );
        for id in [102, 101, 103, 107, 104, 106, 110, 109, 111] {
            tooltips.add(GetDlgItem(hwnd, id));
        }
        app.rebuild_selector();
        app.refresh_history();
        if let Some((x, y)) = app.settings.window {
            let screen = overlay::screen_for(Bounds {
                x,
                y,
                width: 1,
                height: 1,
            });
            let left = x.clamp(
                screen.x,
                (screen.x + screen.width - (bounds.right - bounds.left)).max(screen.x),
            );
            let top = y.clamp(
                screen.y,
                (screen.y + screen.height - (bounds.bottom - bounds.top)).max(screen.y),
            );
            SetWindowPos(hwnd, -1, left, top, 0, 0, 0x11);
        }
        app.status = load_error.unwrap_or_else(|| {
            format!(
                "已恢复 {} 个选区；点击开始可持续识别。",
                app.settings.selections.len()
            )
        });
        let app = Box::new(UiContext {
            tooltips: RefCell::new(tooltips),
            theme: app.theme.clone(),
            dpi: app.dpi,
            app: RefCell::new(*app),
        });
        SetWindowLongPtrW(hwnd, USER_DATA, &*app as *const UiContext as isize);
        SetTimer(hwnd, 1, 150, null());
        ShowWindow(hwnd, 5);
        let mut msg = Msg::default();
        loop {
            let ret = GetMessageW(&mut msg, 0, 0, 0);
            if ret == 0 {
                break;
            }
            if ret < 0 {
                return Err("读取窗口消息失败".to_owned());
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        app.app.borrow_mut().halt();
        Ok(())
    }
}

struct SelectState {
    start: Option<Point>,
    current: Point,
    result: Option<Rect>,
    origin: Point,
}
unsafe extern "system" fn select_proc(hwnd: Handle, msg: u32, w: usize, l: isize) -> isize {
    if !matches!(msg, 0x201 | 0x200 | 0x202 | 0x100 | WM_PAINT) {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let p = GetWindowLongPtrW(hwnd, USER_DATA) as *mut SelectState;
    if p.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let state = &mut *p;
    let mouse = Point {
        x: (l as u32 & 0xffff) as u16 as i16 as i32,
        y: ((l as u32 >> 16) & 0xffff) as u16 as i16 as i32,
    };
    match msg {
        0x201 => {
            state.start = Some(mouse);
            state.current = mouse;
            SetCapture(hwnd);
            0
        }
        0x200 => {
            state.current = mouse;
            if state.start.is_some() {
                InvalidateRect(hwnd, null(), 1);
            }
            0
        }
        0x202 => {
            ReleaseCapture();
            if let Some(start) = state.start {
                state.result = Some(Rect {
                    left: start.x.min(mouse.x) + state.origin.x,
                    top: start.y.min(mouse.y) + state.origin.y,
                    right: start.x.max(mouse.x) + state.origin.x,
                    bottom: start.y.max(mouse.y) + state.origin.y,
                });
            }
            DestroyWindow(hwnd);
            0
        }
        0x100 if w == 27 => {
            DestroyWindow(hwnd);
            0
        }
        WM_PAINT => {
            let mut paint = Paint::default();
            let dc = BeginPaint(hwnd, &mut paint);
            let mut r = Rect::default();
            GetClientRect(hwnd, &mut r);
            FillRect(dc, &r, GetStockObject(4));
            SetTextColor(dc, 0xffffff);
            SetBkMode(dc, 1);
            let text = wide("拖动框选一只宝可梦的名字或 No.编号，松开完成；Esc 取消。");
            r.top += 20;
            DrawTextW(dc, text.as_ptr(), -1, &mut r, 1 | 0x800);
            if let Some(start) = state.start {
                let pen = CreatePen(0, 3, 0x00ffff00);
                let old = SelectObject(dc, pen);
                let brush = SelectObject(dc, GetStockObject(5));
                Rectangle(
                    dc,
                    start.x.min(state.current.x),
                    start.y.min(state.current.y),
                    start.x.max(state.current.x),
                    start.y.max(state.current.y),
                );
                SelectObject(dc, brush);
                SelectObject(dc, old);
                DeleteObject(pen);
            }
            EndPaint(hwnd, &paint);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
unsafe fn choose_roi(parent: Handle) -> Result<Option<Selection>, String> {
    let desktop = desktop_bounds();
    let (width, height) = (desktop.width, desktop.height);
    ShowWindow(parent, 0);
    let origin = Point {
        x: GetSystemMetrics(76),
        y: GetSystemMetrics(77),
    };
    let hwnd = CreateWindowExW(
        0x88 | 0x80000,
        wide("PokeMMOHintSelect").as_ptr(),
        wide("选择名字区域").as_ptr(),
        0x80000000,
        origin.x,
        origin.y,
        GetSystemMetrics(78),
        GetSystemMetrics(79),
        0,
        0,
        GetModuleHandleW(null()),
        null(),
    );
    if hwnd == 0 {
        ShowWindow(parent, 5);
        return Err("无法创建框选窗口".to_owned());
    }
    let mut state = Box::new(SelectState {
        start: None,
        current: Point::default(),
        result: None,
        origin,
    });
    SetWindowLongPtrW(hwnd, USER_DATA, &mut *state as *mut SelectState as isize);
    SetLayeredWindowAttributes(hwnd, 0, 150, 2);
    ShowWindow(hwnd, 5);
    let mut msg = Msg::default();
    while IsWindow(hwnd) != 0 {
        let ret = GetMessageW(&mut msg, 0, 0, 0);
        if ret <= 0 {
            if ret == 0 {
                PostQuitMessage(0);
            }
            break;
        }
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    if IsWindow(hwnd) != 0 {
        DestroyWindow(hwnd);
    }
    ShowWindow(parent, 5);
    if let Some(r) = state.result {
        let roi = Roi {
            x: r.left - origin.x,
            y: r.top - origin.y,
            width: r.right - r.left,
            height: r.bottom - r.top,
        }
        .validate(width, height)?;
        let selection = Selection {
            roi,
            width,
            height,
            origin_x: origin.x,
            origin_y: origin.y,
        };
        selection.screen_bounds(desktop)?;
        return Ok(Some(selection));
    }
    Ok(None)
}

fn settings_directory() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("PokeMMOHint"))
}
fn save_settings(settings: &RegionSettings) -> Result<(), String> {
    let directory = settings_directory().ok_or("无法取得本地配置目录")?;
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let path = directory.join("screen-regions.txt");
    let temporary = directory.join("screen-regions.tmp");
    std::fs::write(&temporary, settings.encode()).map_err(|e| e.to_string())?;
    std::fs::rename(temporary, path).map_err(|e| e.to_string())
}
fn save_history(history: &History) -> Result<(), String> {
    let directory = settings_directory().ok_or("无法取得本地配置目录")?;
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let path = directory.join("recognition-history.txt");
    let temporary = directory.join("recognition-history.tmp");
    std::fs::write(&temporary, history.encode()).map_err(|e| e.to_string())?;
    std::fs::rename(temporary, path).map_err(|e| e.to_string())
}
fn load_history() -> History {
    settings_directory()
        .and_then(|directory| std::fs::read_to_string(directory.join("recognition-history.txt")).ok())
        .map(|text| History::parse(&text))
        .unwrap_or_default()
}
fn load_settings() -> (RegionSettings, Option<String>) {
    let Some(directory) = settings_directory() else {
        return (
            RegionSettings::default(),
            Some("无法取得本地配置目录".to_owned()),
        );
    };
    let path = directory.join("screen-regions.txt");
    if !path.exists() {
        for legacy in ["regions.txt", "roi.txt"] {
            let old = directory.join(legacy);
            if old.exists() {
                let window = std::fs::read_to_string(&old).ok().and_then(|text| {
                    text.lines().find_map(|line| {
                        let value = line.strip_prefix("window=")?;
                        let (x, y) = value.split_once(',')?;
                        Some((x.parse().ok()?, y.parse().ok()?))
                    })
                });
                return (
                    RegionSettings {
                        window,
                        ..RegionSettings::default()
                    },
                    Some("已切换为屏幕选区，请重新框选一次；旧配置文件保留。".to_owned()),
                );
            }
        }
    }
    match std::fs::read_to_string(path) {
        Ok(text) => match RegionSettings::parse(&text) {
            Ok(settings) => (settings, None),
            Err(e) => (
                RegionSettings::default(),
                Some(format!("选区读取失败：{e}")),
            ),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (RegionSettings::default(), None),
        Err(e) => (
            RegionSettings::default(),
            Some(format!("选区读取失败：{e}")),
        ),
    }
}

unsafe fn capture(r: Rect) -> Result<Vec<u8>, String> {
    let width = r.right - r.left;
    let height = r.bottom - r.top;
    let vx = GetSystemMetrics(76);
    let vy = GetSystemMetrics(77);
    if r.left < vx
        || r.top < vy
        || r.right > vx + GetSystemMetrics(78)
        || r.bottom > vy + GetSystemMetrics(79)
    {
        return Err("识别区域超出屏幕，请重新框选".to_owned());
    }
    let source = GetDC(0);
    if source == 0 {
        return Err("无法取得屏幕画面".to_owned());
    }
    let dest = CreateCompatibleDC(source);
    if dest == 0 {
        ReleaseDC(0, source);
        return Err("无法创建截图缓冲".to_owned());
    }
    let info = BitmapInfo {
        header: BitmapHeader {
            size: 40,
            width,
            height: -height,
            planes: 1,
            bits: 32,
            ..Default::default()
        },
        colors: [0],
    };
    let mut pixels = null_mut();
    let bitmap = CreateDIBSection(source, &info, 0, &mut pixels, 0, 0);
    if bitmap == 0 || pixels.is_null() {
        DeleteDC(dest);
        ReleaseDC(0, source);
        return Err("无法分配截图缓冲".to_owned());
    }
    let previous = SelectObject(dest, bitmap);
    let ok = BitBlt(
        dest,
        0,
        0,
        width,
        height,
        source,
        r.left,
        r.top,
        0x00cc0020 | 0x40000000,
    );
    GdiFlush();
    let result = if ok != 0 {
        Ok(std::slice::from_raw_parts(pixels as *const u8, (width * height * 4) as usize).to_vec())
    } else {
        Err("截取画面失败，请用窗口或无边框模式".to_owned())
    };
    SelectObject(dest, previous);
    DeleteObject(bitmap);
    DeleteDC(dest);
    ReleaseDC(0, source);
    result
}

struct Scratch {
    directory: PathBuf,
    image: PathBuf,
    alternate: PathBuf,
    white: PathBuf,
    windows_script: PathBuf,
}
impl Scratch {
    fn create() -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "pokemmo-hint-{}-{stamp}-{:?}",
            std::process::id(),
            thread::current().id()
        ));
        std::fs::create_dir(&directory).map_err(|e| e.to_string())?;
        let scratch = Self {
            image: directory.join("capture.bmp"),
            alternate: directory.join("alternate.bmp"),
            white: directory.join("white-text.bmp"),
            windows_script: directory.join("windows-ocr.ps1"),
            directory,
        };
        std::fs::write(&scratch.windows_script, include_str!("windows_ocr.ps1"))
            .map_err(|e| e.to_string())?;
        Ok(scratch)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

struct WorkerContext {
    config: Backend,
    refresh_interval: Duration,
    own: Handle,
    stop: Arc<AtomicBool>,
    tx: mpsc::Sender<Update>,
    confirmed: Arc<Mutex<HashMap<String, u16>>>,
    occlusions: Arc<Mutex<Vec<Rect>>>,
}
fn watch(context: WorkerContext, selections: Vec<(usize, Selection)>) {
    let WorkerContext {
        config,
        refresh_interval,
        own,
        stop,
        tx,
        confirmed,
        occlusions,
    } = context;
    let scratch = match Scratch::create() {
        Ok(s) => s,
        Err(e) => {
            for (index, _) in &selections {
                let _ = tx.send(Update {
                    index: *index,
                    deferred: false,
                    status: e.clone(),
                    body: "尚未识别".to_owned(),
                    card: None,
                    suggestion: None,
                });
            }
            return;
        }
    };
    let data = species_data();
    let mut trackers: Vec<Tracker> = selections.iter().map(|_| Tracker::default()).collect();

    while !stop.load(Ordering::Relaxed) {
        let round_started = Instant::now();
        for ((index, selected), tracker) in selections.iter().zip(trackers.iter_mut()) {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let mut deferred = false;
            let result = (|| -> Result<pokemmo_hint::ReadDecision, String> {
                let pixels = unsafe {
                    let region = screen_rect(*selected)?;
                    let mut own_rect = Rect::default();
                    if IsIconic(own) == 0
                        && GetWindowRect(own, &mut own_rect) != 0
                        && intersects(region, own_rect)
                    {
                        return Err("提示窗口挡住了识别区域，请拖开或最小化。".to_owned());
                    }
                    if occlusions
                        .lock()
                        .map_err(|_| "浮窗位置不可用")?
                        .iter()
                        .any(|r| intersects(region, *r))
                    {
                        deferred = true;
                        return Err("浮动信息卡遮挡，暂缓读取此选区".to_owned());
                    }
                    capture(region)?
                };
                std::fs::write(
                    &scratch.image,
                    ocr_bitmap(&pixels, selected.roi.width, selected.roi.height),
                )
                .map_err(|e| e.to_string())?;
                let alternate = if let Some(white) =
                    white_text_bitmap(&pixels, selected.roi.width, selected.roi.height)
                {
                    std::fs::write(&scratch.white, white).map_err(|e| e.to_string())?;
                    Some(scratch.white.as_path())
                } else {
                    None
                };
                let raw =
                    config.recognize(&scratch.image, alternate, &scratch.windows_script, &stop)?;
                let aliases = confirmed.lock().map_err(|_| "确认记录不可用")?.clone();
                let mut reads = raw.lines().map(str::to_owned).collect::<Vec<_>>();
                let mut decision = resolve_reads(&reads, &data, &aliases);
                if decision.result == MatchResult::Unknown && !stop.load(Ordering::Relaxed) {
                    std::fs::write(
                        &scratch.alternate,
                        binary_bitmap(&pixels, selected.roi.width, selected.roi.height),
                    )
                    .map_err(|e| e.to_string())?;
                    if let Ok(raw) =
                        config.recognize(&scratch.alternate, None, &scratch.windows_script, &stop)
                    {
                        reads.extend(raw.lines().map(str::to_owned));
                        decision = resolve_reads(&reads, &data, &aliases);
                    }
                }
                Ok(decision)
            })();
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let update = match result {
                Ok(decision) => {
                    let stable = tracker.update(&decision.result);
                    let card = stable
                        .and_then(|id| data.iter().find(|s| s.id == id))
                        .map(hint_card);
                    let body = if card.is_some() {
                        String::new()
                    } else if let Some(suggestion) = &decision.suggestion {
                        let name = data
                            .iter()
                            .find(|s| s.id == suggestion.id)
                            .map(|s| s.name.as_str())
                            .unwrap_or("");
                        format!(
                            "识别文字：{}\r\n可能是「{}」。确认后，本次运行会记住这个读法。",
                            suggestion.observed, name
                        )
                    } else {
                        match decision.result {
                            MatchResult::Found(_) => "发现物种，正在等待下一次一致结果…".to_owned(),
                            MatchResult::Unknown => {
                                "没有匹配到名字或编号。\r\n请只框文字那一行，并在四周稍留空隙。"
                                    .to_owned()
                            }
                            MatchResult::Ambiguous => "识别到多个候选，请缩小识别区域。".to_owned(),
                            MatchResult::Conflict => "编号与名字不一致，请重新识别。".to_owned(),
                        }
                    };
                    let preview: String = decision
                        .raw
                        .chars()
                        .take(80)
                        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
                        .collect();
                    let status = format!(
                        "识别文字：{}{}",
                        preview,
                        if decision.corrected {
                            " · 使用已确认读法"
                        } else {
                            ""
                        }
                    );
                    Update {
                        index: *index,
                        deferred: false,
                        status,
                        body,
                        card,
                        suggestion: decision.suggestion,
                    }
                }
                Err(error) => {
                    if !deferred {
                        tracker.update(&MatchResult::Unknown);
                    }
                    Update {
                        index: *index,
                        deferred,
                        status: error,
                        body: "暂无可靠结果".to_owned(),
                        card: None,
                        suggestion: None,
                    }
                }
            };
            if tx.send(update).is_err() {
                return;
            }
        }
        while !stop.load(Ordering::Relaxed) {
            let remaining = refresh_interval.saturating_sub(round_started.elapsed());
            if remaining.is_zero() {
                break;
            }
            thread::sleep(remaining.min(Duration::from_millis(50)));
        }
    }
}
