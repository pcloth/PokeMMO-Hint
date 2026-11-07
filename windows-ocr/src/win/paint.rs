use super::*;
use pokemmo_hint::{TypeEffect, TYPE_NAMES};

pub(super) fn px(value: i32, dpi: u32) -> i32 {
    (value as i64 * dpi as i64 / 96) as i32
}
const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}
const BG: u32 = rgb(243, 246, 251);
const INK: u32 = rgb(30, 41, 59);
const MUTED: u32 = rgb(100, 116, 139);
const BORDER: u32 = rgb(224, 231, 240);
const BLUE: u32 = rgb(37, 99, 235);
const WHITE: u32 = rgb(255, 255, 255);

pub(super) struct Theme {
    pub body: Handle,
    small: Handle,
    title: Handle,
    white: Handle,
    buttons: Vec<Handle>,
}
impl Theme {
    pub unsafe fn new(dpi: u32) -> Self {
        let font = |size, weight| {
            CreateFontW(
                -px(size, dpi),
                0,
                0,
                0,
                weight,
                0,
                0,
                0,
                1,
                0,
                0,
                5,
                0,
                wide("Microsoft YaHei UI").as_ptr(),
            )
        };
        Self {
            body: font(15, 400),
            small: font(12, 400),
            title: font(23, 600),
            white: CreateSolidBrush(WHITE),
            buttons: (10..=22)
                .map(|id| {
                    LoadImageW(
                        GetModuleHandleW(null()),
                        integer_resource(id),
                        1,
                        px(28, dpi),
                        px(28, dpi),
                        0x8000,
                    )
                })
                .collect(),
        }
    }
    pub fn white_brush(&self) -> Handle {
        self.white
    }
}
impl Drop for Theme {
    fn drop(&mut self) {
        unsafe {
            for obj in [self.body, self.small, self.title, self.white] {
                if obj != 0 {
                    DeleteObject(obj);
                }
            }
        }
    }
}

unsafe fn rounded(dc: Handle, r: Rect, fill: u32, edge: u32, radius: i32) {
    let brush = CreateSolidBrush(fill);
    let pen = CreatePen(0, 1, edge);
    let oldb = SelectObject(dc, brush);
    let oldp = SelectObject(dc, pen);
    RoundRect(dc, r.left, r.top, r.right, r.bottom, radius, radius);
    SelectObject(dc, oldp);
    SelectObject(dc, oldb);
    DeleteObject(pen);
    DeleteObject(brush);
}
pub(super) unsafe fn text(
    dc: Handle,
    font: Handle,
    color: u32,
    value: &str,
    mut r: Rect,
    flags: u32,
) {
    let old = SelectObject(dc, font);
    SetBkMode(dc, 1);
    SetTextColor(dc, color);
    let value = wide(value);
    DrawTextW(dc, value.as_ptr(), -1, &mut r, flags | 0x800);
    SelectObject(dc, old);
}
unsafe fn measure(dc: Handle, font: Handle, value: &str) -> i32 {
    let old = SelectObject(dc, font);
    let value = wide(value);
    let mut size = Point::default();
    GetTextExtentPoint32W(dc, value.as_ptr(), value.len() as i32 - 1, &mut size);
    SelectObject(dc, old);
    size.x
}
pub(super) fn rect(dpi: u32, x: i32, y: i32, w: i32, h: i32) -> Rect {
    Rect {
        left: px(x, dpi),
        top: px(y, dpi),
        right: px(x + w, dpi),
        bottom: px(y + h, dpi),
    }
}
struct CardView<'a> {
    theme: &'a Theme,
    dpi: u32,
}

unsafe fn chip(
    dc: Handle,
    app: &CardView<'_>,
    label: &str,
    x: i32,
    y: i32,
    fill: u32,
    color: u32,
) -> i32 {
    let dpi = app.dpi;
    let width = measure(dc, app.theme.body, label) + px(14, dpi);
    let r = Rect {
        left: x,
        top: y,
        right: x + width,
        bottom: y + px(29, dpi),
    };
    rounded(dc, r, fill, fill, px(9, dpi));
    text(dc, app.theme.body, color, label, r, 1 | 4 | 0x20);
    width
}
fn effect_label(item: &TypeEffect) -> String {
    let suffix = match item.multiplier {
        4.0 => "×4",
        2.0 => "×2",
        0.5 => "×½",
        0.25 => "×¼",
        _ => "",
    };
    format!("{}{}", TYPE_NAMES[item.type_id], suffix)
}
unsafe fn effects_end(dc: Handle, theme: &Theme, dpi: u32, items: &[TypeEffect], y: i32) -> i32 {
    let (mut x, mut line) = (px(106, dpi), px(y, dpi));
    for item in items {
        let width = measure(dc, theme.body, &effect_label(item)) + px(14, dpi);
        if x + width > px(578, dpi) {
            x = px(106, dpi);
            line += px(35, dpi);
        }
        x += width + px(5, dpi);
    }
    (line * 96 / dpi as i32) + 40
}
pub(super) unsafe fn card_height(theme: &Theme, dpi: u32, card: &HintCard) -> i32 {
    let dc = GetDC(0);
    let mut y = 215;
    for items in [
        &card.weaknesses,
        &card.neutral,
        &card.resistances,
        &card.immunities,
    ] {
        y = effects_end(dc, theme, dpi, items, y) + 7;
    }
    ReleaseDC(0, dc);
    y - 7 - 136 + 12
}
unsafe fn effects(
    dc: Handle,
    app: &CardView<'_>,
    label: &str,
    items: &[TypeEffect],
    y: i32,
    fill: u32,
    color: u32,
) -> i32 {
    let dpi = app.dpi;
    let mut x = px(106, dpi);
    let mut line = px(y, dpi);
    text(
        dc,
        app.theme.body,
        color,
        label,
        rect(dpi, 40, y, 60, 30),
        4 | 0x20,
    );
    if items.is_empty() {
        text(
            dc,
            app.theme.body,
            MUTED,
            "无",
            rect(dpi, 106, y, 430, 30),
            4 | 0x20,
        );
        return y + 40;
    }
    for item in items {
        let label = effect_label(item);
        let width = measure(dc, app.theme.body, &label) + px(14, dpi);
        if x + width > px(578, dpi) {
            x = px(106, dpi);
            line += px(35, dpi);
        }
        x += chip(dc, app, &label, x, line, fill, color) + px(5, dpi);
    }
    (line * 96 / dpi as i32) + 40
}

pub(super) unsafe fn draw(app: &App, dc: Handle, area: Rect) {
    let brush = CreateSolidBrush(BG);
    FillRect(dc, &area, brush);
    DeleteObject(brush);
    let dpi = app.dpi;
    text(
        dc,
        app.theme.title,
        INK,
        "属性速查",
        rect(dpi, 20, 13, 170, 35),
        0x20,
    );
    text(
        dc,
        app.theme.small,
        MUTED,
        &format!(
            "{} 个选区 · {}",
            app.settings.selections.len(),
            app.engine_settings.engine.label()
        ),
        rect(dpi, 195, 24, 250, 22),
        0x20,
    );
    let state = if !app.workers.is_empty() {
        "● 识别中"
    } else {
        "○ 已暂停"
    };
    text(
        dc,
        app.theme.small,
        if !app.workers.is_empty() {
            rgb(5, 150, 105)
        } else {
            MUTED
        },
        state,
        rect(dpi, 500, 24, 100, 22),
        2 | 0x20,
    );
    text(
        dc,
        app.theme.small,
        MUTED,
        &app.status,
        rect(dpi, 314, 104, 284, 23),
        0x20 | 0x8000,
    );
    if let Some(card) = &app.card {
        draw_card(
            dc,
            &app.theme,
            app.dpi,
            card,
            px(20, app.dpi),
            px(136, app.dpi),
        );
    } else {
        rounded(
            dc,
            rect(dpi, 20, 136, 580, app.card_height),
            WHITE,
            BORDER,
            px(18, dpi),
        );
        let title = if app.suggestion.is_some() {
            "找到一个可能的物种"
        } else if !app.workers.is_empty() {
            "等待可靠的识别结果"
        } else {
            "准备好后，开始识别"
        };
        text(
            dc,
            app.theme.title,
            INK,
            title,
            rect(dpi, 42, 191, 536, 40),
            1 | 0x20,
        );
        text(
            dc,
            app.theme.body,
            MUTED,
            &app.body,
            rect(dpi, 55, 245, 510, 105),
            1 | 0x10,
        );
    }
    text(
        dc,
        app.theme.small,
        MUTED,
        "手动查询",
        rect(dpi, 22, 410 + app.card_height - 258, 120, 19),
        0x20,
    );
    if app.suggestion.is_none() {
        text(
            dc,
            app.theme.small,
            MUTED,
            "支持中文名字、No.573 或全国编号",
            rect(dpi, 327, 442, 271, 22),
            0x20 | 0x8000,
        );
    }
}

#[repr(C)]
pub(super) struct DrawItem {
    kind: u32,
    id: u32,
    item: u32,
    action: u32,
    state: u32,
    hwnd: Handle,
    dc: Handle,
    area: Rect,
    data: usize,
}
pub(super) unsafe fn button(item: &DrawItem, theme: &Theme, dpi: u32) {
    let primary = item.id == 102;
    let fill = if item.state & 4 != 0 {
        rgb(235, 239, 245)
    } else if item.state & 1 != 0 {
        rgb(214, 229, 250)
    } else if primary {
        rgb(232, 242, 255)
    } else {
        WHITE
    };
    let mut r = item.area;
    r.right -= 1;
    r.bottom -= 1;
    rounded(
        item.dc,
        r,
        fill,
        if primary { rgb(180, 210, 250) } else { BORDER },
        px(9, dpi),
    );
    let index = match item.id {
        102 => usize::from(get_text(item.hwnd) == "暂停识别"),
        101 => 2,
        103 => 3,
        107 => 4,
        104 => 5,
        106 => 6,
        110 => 7,
        109 => 8,
        111 => 9,
        201 => 6,
        202 => 10,
        203 => 11,
        204 => 12,
        _ => 0,
    };
    let size = px(
        if r.bottom - r.top < px(33, dpi) {
            24
        } else {
            28
        },
        dpi,
    );
    if let Some(&icon) = theme.buttons.get(index).filter(|&&h| h != 0) {
        DrawIconEx(
            item.dc,
            (r.left + r.right - size) / 2,
            (r.top + r.bottom - size) / 2,
            icon,
            size,
            size,
            0,
            0,
            3,
        );
    }
    if item.state & 0x10 != 0 {
        let focus = Rect {
            left: r.left + 3,
            top: r.top + 3,
            right: r.right - 3,
            bottom: r.bottom - 3,
        };
        DrawFocusRect(item.dc, &focus);
    }
}

pub(super) unsafe fn paint_window(hwnd: Handle, app: Option<&App>) {
    let mut paint = Paint::default();
    let dc = BeginPaint(hwnd, &mut paint);
    if let Some(app) = app {
        let mut area = Rect::default();
        GetClientRect(hwnd, &mut area);
        let buffer = CreateCompatibleDC(dc);
        let bitmap = CreateCompatibleBitmap(dc, area.right, area.bottom);
        if buffer != 0 && bitmap != 0 {
            let old = SelectObject(buffer, bitmap);
            draw(app, buffer, area);
            BitBlt(dc, 0, 0, area.right, area.bottom, buffer, 0, 0, 0x00cc0020);
            SelectObject(buffer, old);
        } else {
            draw(app, dc, area);
        }
        if bitmap != 0 {
            DeleteObject(bitmap);
        }
        if buffer != 0 {
            DeleteDC(buffer);
        }
    }
    EndPaint(hwnd, &paint);
}

pub(super) unsafe fn draw_card(
    dc: Handle,
    theme: &Theme,
    dpi: u32,
    card: &HintCard,
    x: i32,
    y: i32,
) {
    let app = &CardView { theme, dpi };
    let saved = SaveDC(dc);
    SetViewportOrgEx(dc, x - px(20, dpi), y - px(136, dpi), null_mut());
    rounded(
        dc,
        rect(dpi, 20, 136, 580, card_height(theme, dpi, card)),
        WHITE,
        BORDER,
        px(18, dpi),
    );
    text(
        dc,
        app.theme.title,
        INK,
        &format!("#{:03}  {}", card.id, card.name),
        rect(dpi, 40, 150, 375, 40),
        0x20 | 0x8000,
    );
    let mut right = px(578, dpi);
    for &kind in card.types.iter().rev() {
        let label = TYPE_NAMES[kind];
        let width = measure(dc, app.theme.body, label) + px(14, dpi);
        let color = match kind {
            9 => rgb(185, 28, 28),
            10 => rgb(29, 78, 216),
            11 => rgb(21, 128, 61),
            12 => rgb(161, 98, 7),
            _ => rgb(67, 56, 202),
        };
        chip(
            dc,
            app,
            label,
            right - width,
            px(155, dpi),
            rgb(237, 242, 250),
            color,
        );
        right -= width + px(5, dpi);
    }
    let brush = CreateSolidBrush(BORDER);
    FillRect(dc, &rect(dpi, 40, 200, 540, 1), brush);
    DeleteObject(brush);
    let y = effects(
        dc,
        app,
        "弱点",
        &card.weaknesses,
        215,
        rgb(255, 239, 239),
        rgb(185, 28, 28),
    );
    let y = effects(
        dc,
        app,
        "普通伤害",
        &card.neutral,
        y + 7,
        rgb(241, 245, 249),
        MUTED,
    );
    let y = effects(
        dc,
        app,
        "抗性",
        &card.resistances,
        y + 7,
        rgb(232, 248, 242),
        rgb(4, 120, 87),
    );
    effects(
        dc,
        app,
        "免疫",
        &card.immunities,
        y + 7,
        rgb(242, 238, 255),
        rgb(109, 40, 217),
    );
    RestoreDC(dc, saved);
}
pub(super) unsafe fn label_size(theme: &Theme, dpi: u32, label: &str) -> (i32, i32) {
    let dc = GetDC(0);
    let w = measure(dc, theme.body, label) + px(24, dpi);
    ReleaseDC(0, dc);
    (w.max(px(65, dpi)), px(30, dpi))
}
pub(super) unsafe fn draw_tag(dc: Handle, theme: &Theme, dpi: u32, label: &str, area: Rect) {
    let fill = CreateSolidBrush(0xff00ff);
    FillRect(dc, &area, fill);
    DeleteObject(fill);
    let mut r = area;
    r.right -= 1;
    r.bottom -= 1;
    rounded(dc, r, rgb(255, 239, 239), rgb(250, 200, 200), px(10, dpi));
    text(dc, theme.body, rgb(185, 28, 28), label, r, 1 | 4 | 0x20);
}

pub(super) unsafe fn draw_tooltip(dc: Handle, theme: &Theme, dpi: u32, label: &str, area: Rect) {
    let brush = CreateSolidBrush(0xff00ff);
    FillRect(dc, &area, brush);
    DeleteObject(brush);
    let mut r = area;
    r.right -= 1;
    r.bottom -= 1;
    rounded(dc, r, rgb(30, 41, 59), rgb(51, 65, 85), px(8, dpi));
    text(dc, theme.body, WHITE, label, r, 1 | 4 | 0x20 | 0x8000);
}
