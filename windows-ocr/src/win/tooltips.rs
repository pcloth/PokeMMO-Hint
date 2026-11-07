//! App-painted tooltips avoid common-controls v5/v6 TOOLINFO size differences.
use super::*;
struct Context {
    theme: Rc<paint::Theme>,
    dpi: u32,
    text: RefCell<String>,
}
pub(super) struct Tooltips {
    hwnd: Handle,
    parent: Handle,
    context: Box<Context>,
    buttons: Vec<Handle>,
    hovered: Option<Handle>,
    since: Instant,
}
impl Tooltips {
    pub(super) unsafe fn new(parent: Handle, theme: Rc<paint::Theme>, dpi: u32) -> Self {
        let context = Box::new(Context {
            theme,
            dpi,
            text: RefCell::new(String::new()),
        });
        let hwnd = CreateWindowExW(
            0x080800a8,
            wide("PokeMMOHintButtonTip").as_ptr(),
            wide("").as_ptr(),
            0x80000000,
            0,
            0,
            1,
            1,
            parent,
            0,
            GetModuleHandleW(null()),
            null(),
        );
        if hwnd != 0 {
            SetWindowLongPtrW(hwnd, USER_DATA, (&*context as *const Context) as isize);
            SetLayeredWindowAttributes(hwnd, 0xff00ff, 255, 1);
        }
        Self {
            hwnd,
            parent,
            context,
            buttons: Vec::new(),
            hovered: None,
            since: Instant::now(),
        }
    }
    pub(super) fn add(&mut self, button: Handle) {
        if button != 0 {
            self.buttons.push(button);
        }
    }
    unsafe fn hide(&mut self) {
        if self.hwnd != 0 {
            ShowWindow(self.hwnd, 0);
        }
    }
    pub(super) unsafe fn tick(&mut self) {
        if self.hwnd == 0 {
            return;
        }
        let active = GetForegroundWindow() == self.parent
            && IsIconic(self.parent) == 0
            && IsWindowVisible(self.parent) != 0;
        let mut cursor = Point::default();
        GetCursorPos(&mut cursor);
        let hovered = if active {
            self.buttons.iter().copied().find(|button| {
                let mut rect = Rect::default();
                IsWindowVisible(*button) != 0
                    && GetWindowRect(*button, &mut rect) != 0
                    && overlay::bounds(rect).contains(cursor.x, cursor.y)
            })
        } else {
            None
        };
        if hovered != self.hovered {
            self.hovered = hovered;
            self.since = Instant::now();
            self.hide();
        }
        let Some(button) = hovered else {
            self.hide();
            return;
        };
        if self.since.elapsed() < Duration::from_millis(300) {
            return;
        }
        // Read the accessible button label on every tick: play/pause and candidate names stay current.
        let label = get_text(button);
        if label.is_empty() {
            self.hide();
            return;
        }
        if *self.context.text.borrow() != label {
            *self.context.text.borrow_mut() = label.clone();
            InvalidateRect(self.hwnd, null(), 0);
        }
        let (width, height) = paint::label_size(&self.context.theme, self.context.dpi, &label);
        let mut parent = Rect::default();
        let mut rect = Rect::default();
        GetWindowRect(self.parent, &mut parent);
        GetWindowRect(button, &mut rect);
        // Keep hints inside the main window's bounds, already excluded from OCR captures.
        let width = width.min((parent.right - parent.left - 16).max(1));
        let x = (rect.left + 4).clamp(
            parent.left + 8,
            (parent.right - width - 8).max(parent.left + 8),
        );
        let y = (rect.bottom + 5).clamp(
            parent.top + 8,
            (parent.bottom - height - 8).max(parent.top + 8),
        );
        SetWindowPos(self.hwnd, -1, x, y, width, height, 0x50);
    }
}
impl Drop for Tooltips {
    fn drop(&mut self) {
        unsafe {
            if IsWindow(self.hwnd) != 0 {
                SetWindowLongPtrW(self.hwnd, USER_DATA, 0);
                DestroyWindow(self.hwnd);
            }
        }
    }
}
pub(super) unsafe extern "system" fn window_proc(
    hwnd: Handle,
    msg: u32,
    w: usize,
    l: isize,
) -> isize {
    if msg == 0x21 {
        return 3;
    }
    if msg == 0x14 {
        return 1;
    }
    if msg != WM_PAINT {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let mut ps = Paint::default();
    let dc = BeginPaint(hwnd, &mut ps);
    let p = GetWindowLongPtrW(hwnd, USER_DATA) as *const Context;
    if !p.is_null() {
        let ctx = &*p;
        let mut area = Rect::default();
        GetClientRect(hwnd, &mut area);
        if let Ok(label) = ctx.text.try_borrow() {
            paint::draw_tooltip(dc, &ctx.theme, ctx.dpi, &label, area);
        }
    }
    EndPaint(hwnd, &ps);
    0
}
