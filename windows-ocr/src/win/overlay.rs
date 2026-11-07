use super::*;

pub(super) fn bounds(r: Rect) -> Bounds {
    Bounds {
        x: r.left,
        y: r.top,
        width: r.right - r.left,
        height: r.bottom - r.top,
    }
}
pub(super) fn rect(r: Bounds) -> Rect {
    Rect {
        left: r.x,
        top: r.y,
        right: r.x + r.width,
        bottom: r.y + r.height,
    }
}
pub(super) unsafe fn screen_for(r: Bounds) -> Bounds {
    let monitor = MonitorFromPoint(
        Point {
            x: r.x + r.width / 2,
            y: r.y + r.height / 2,
        },
        2,
    );
    let mut info = MonitorInfo {
        size: std::mem::size_of::<MonitorInfo>() as u32,
        monitor: Rect::default(),
        work: Rect::default(),
        flags: 0,
    };
    if GetMonitorInfoW(monitor, &mut info) != 0 {
        bounds(info.work)
    } else {
        Bounds {
            x: GetSystemMetrics(76),
            y: GetSystemMetrics(77),
            width: GetSystemMetrics(78),
            height: GetSystemMetrics(79),
        }
    }
}
struct Content {
    card: HintCard,
    label: String,
}
struct Context {
    theme: Rc<paint::Theme>,
    dpi: u32,
    full: bool,
    content: RefCell<Content>,
}
pub(super) struct Window {
    hwnd: Handle,
    context: Box<Context>,
}
impl Window {
    pub(super) unsafe fn new(
        theme: Rc<paint::Theme>,
        dpi: u32,
        card: &HintCard,
        full: bool,
    ) -> Result<Self, String> {
        let context = Box::new(Context {
            theme,
            dpi,
            full,
            content: RefCell::new(Content {
                card: card.clone(),
                label: pokemmo_hint::regions::first_weakness(card),
            }),
        });
        let hwnd = CreateWindowExW(
            0x08080088,
            wide("PokeMMOHintOverlay").as_ptr(),
            wide("").as_ptr(),
            0x80000000,
            0,
            0,
            1,
            1,
            0,
            0,
            GetModuleHandleW(null()),
            null(),
        );
        if hwnd == 0 {
            return Err("无法创建浮动标签".to_owned());
        }
        SetWindowLongPtrW(hwnd, USER_DATA, (&*context as *const Context) as isize);
        SetLayeredWindowAttributes(hwnd, 0xff00ff, 255, 1);
        SetWindowDisplayAffinity(hwnd, 0x11);
        Ok(Self { hwnd, context })
    }
    pub(super) unsafe fn set_card(&mut self, card: &HintCard) {
        let mut content = self.context.content.borrow_mut();
        if content.card.id != card.id {
            content.card = card.clone();
            content.label = pokemmo_hint::regions::first_weakness(card);
            InvalidateRect(self.hwnd, null(), 0);
        }
    }
    pub(super) unsafe fn size(&self) -> (i32, i32) {
        if self.context.full {
            (
                paint::px(600, self.context.dpi),
                paint::px(
                    paint::card_height(
                        &self.context.theme,
                        self.context.dpi,
                        &self.context.content.borrow().card,
                    ) + 20,
                    self.context.dpi,
                ),
            )
        } else {
            paint::label_size(
                &self.context.theme,
                self.context.dpi,
                &self.context.content.borrow().label,
            )
        }
    }
    pub(super) unsafe fn show_at(&self, r: Bounds) {
        if self.visible_bounds() != Some(r) {
            SetWindowPos(self.hwnd, -1, r.x, r.y, r.width, r.height, 0x50);
        }
    }
    pub(super) unsafe fn hide(&self) {
        if IsWindowVisible(self.hwnd) != 0 {
            ShowWindow(self.hwnd, 0);
        }
    }
    pub(super) unsafe fn visible_bounds(&self) -> Option<Bounds> {
        let mut r = Rect::default();
        (IsWindowVisible(self.hwnd) != 0 && GetWindowRect(self.hwnd, &mut r) != 0)
            .then(|| bounds(r))
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            SetWindowLongPtrW(self.hwnd, USER_DATA, 0);
            DestroyWindow(self.hwnd);
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
    } // MA_NOACTIVATE: hovering never takes focus from the game.
    if msg == 0x14 {
        return 1;
    }
    if msg != WM_PAINT {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let p = GetWindowLongPtrW(hwnd, USER_DATA) as *const Context;
    let mut ps = Paint::default();
    let dc = BeginPaint(hwnd, &mut ps);
    if !p.is_null() {
        let ctx = &*p;
        let mut area = Rect::default();
        GetClientRect(hwnd, &mut area);
        let brush = CreateSolidBrush(0xff00ff);
        FillRect(dc, &area, brush);
        DeleteObject(brush);
        if let Ok(content) = ctx.content.try_borrow() {
            if ctx.full {
                paint::draw_card(
                    dc,
                    &ctx.theme,
                    ctx.dpi,
                    &content.card,
                    paint::px(10, ctx.dpi),
                    paint::px(10, ctx.dpi),
                );
            } else {
                paint::draw_tag(dc, &ctx.theme, ctx.dpi, &content.label, area);
            }
        }
    }
    EndPaint(hwnd, &ps);
    0
}
