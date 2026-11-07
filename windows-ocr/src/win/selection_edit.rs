use super::*;
use pokemmo_hint::regions::drag_roi;

struct State {
    roi: Roi,
    others: Vec<Roi>,
    name: String,
    width: i32,
    height: i32,
    drag: Option<(Point, Roi, u8)>,
    accepted: bool,
}
fn hit(roi: Roi, p: Point) -> Option<u8> {
    let margin = 6;
    if p.x < roi.x - margin
        || p.x > roi.x + roi.width + margin
        || p.y < roi.y - margin
        || p.y > roi.y + roi.height + margin
    {
        return None;
    }
    let mut edges = 0;
    if (p.x - roi.x).abs() <= margin {
        edges |= 1;
    } else if (p.x - roi.x - roi.width).abs() <= margin {
        edges |= 2;
    }
    if (p.y - roi.y).abs() <= margin {
        edges |= 4;
    } else if (p.y - roi.y - roi.height).abs() <= margin {
        edges |= 8;
    }
    Some(edges)
}
pub(super) unsafe extern "system" fn window_proc(
    hwnd: Handle,
    msg: u32,
    w: usize,
    l: isize,
) -> isize {
    if !matches!(msg, WM_PAINT | 0x201 | 0x202 | 0x200 | 0x100 | 0x20 | 0x215) {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    let p = GetWindowLongPtrW(hwnd, USER_DATA) as *const RefCell<State>;
    if p.is_null() {
        return DefWindowProcW(hwnd, msg, w, l);
    }
    if msg == WM_PAINT {
        let mut ps = Paint::default();
        let dc = BeginPaint(hwnd, &mut ps);
        let mut area = Rect::default();
        GetClientRect(hwnd, &mut area);
        FillRect(dc, &area, GetStockObject(4));
        if let Ok(state) = (&*p).try_borrow() {
            let oldb = SelectObject(dc, GetStockObject(5));
            for (roi, color, thick) in state
                .others
                .iter()
                .map(|r| (*r, 0x999999, 1))
                .chain(std::iter::once((state.roi, 0x00c8ff, 3)))
            {
                let pen = CreatePen(0, thick, color);
                let oldp = SelectObject(dc, pen);
                Rectangle(dc, roi.x, roi.y, roi.x + roi.width, roi.y + roi.height);
                SelectObject(dc, oldp);
                DeleteObject(pen);
            }
            SelectObject(dc, oldb);
            SetBkMode(dc, 1);
            SetTextColor(dc, 0xffffff);
            let text = wide(&format!(
                "{}：拖动框内移动，拖动边缘或角调整大小；Enter 保存，Esc 取消",
                state.name
            ));
            area.top += 20;
            DrawTextW(dc, text.as_ptr(), -1, &mut area, 1 | 0x800);
            let r = state.roi;
            for (x, y) in [
                (r.x, r.y),
                (r.x + r.width, r.y),
                (r.x, r.y + r.height),
                (r.x + r.width, r.y + r.height),
            ] {
                let brush = CreateSolidBrush(0x00c8ff);
                FillRect(
                    dc,
                    &Rect {
                        left: x - 4,
                        top: y - 4,
                        right: x + 4,
                        bottom: y + 4,
                    },
                    brush,
                );
                DeleteObject(brush);
            }
        }
        EndPaint(hwnd, &ps);
        return 0;
    }
    if msg == 0x20 {
        let mut cursor = Point::default();
        GetCursorPos(&mut cursor);
        ScreenToClient(hwnd, &mut cursor);
        if let Ok(state) = (&*p).try_borrow() {
            let edges = state
                .drag
                .map(|(_, _, e)| e)
                .or_else(|| hit(state.roi, cursor));
            let id = match edges {
                Some(0) => 32646,
                Some(1 | 2) => 32644,
                Some(4 | 8) => 32645,
                Some(5 | 10) => 32642,
                Some(6 | 9) => 32643,
                _ => 32512,
            };
            SetCursor(LoadCursorW(0, integer_resource(id)));
            return 1;
        }
    }
    let Ok(mut state) = (&*p).try_borrow_mut() else {
        return DefWindowProcW(hwnd, msg, w, l);
    };
    let mouse = Point {
        x: (l as u32 & 0xffff) as u16 as i16 as i32,
        y: ((l as u32 >> 16) & 0xffff) as u16 as i16 as i32,
    };
    match msg {
        0x201 => {
            if let Some(edges) = hit(state.roi, mouse) {
                state.drag = Some((mouse, state.roi, edges));
                SetCapture(hwnd);
            }
            0
        }
        0x200 => {
            if let Some((start, original, edges)) = state.drag {
                state.roi = drag_roi(
                    original,
                    mouse.x - start.x,
                    mouse.y - start.y,
                    edges,
                    state.width,
                    state.height,
                );
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        0x202 => {
            state.drag = None;
            ReleaseCapture();
            0
        }
        0x215 => {
            state.drag = None;
            0
        }
        0x100 if w == 13 => {
            state.accepted = true;
            DestroyWindow(hwnd);
            0
        }
        0x100 if w == 27 => {
            DestroyWindow(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
pub(super) unsafe fn edit(
    parent: Handle,
    selected: Selection,
    name: String,
    others: Vec<Roi>,
) -> Result<Option<Selection>, String> {
    let game = game_window()?;
    if client_size(game)? != (selected.width, selected.height) {
        return Err("游戏尺寸改变，请删除旧选区后重新添加。".to_owned());
    }
    let mut origin = Point::default();
    if ClientToScreen(game, &mut origin) == 0 {
        return Err("无法取得游戏位置".to_owned());
    }
    ShowWindow(parent, 0);
    let hwnd = CreateWindowExW(
        0x80088,
        wide("PokeMMOHintEditRegion").as_ptr(),
        wide("调整选区").as_ptr(),
        0x80000000,
        origin.x,
        origin.y,
        selected.width,
        selected.height,
        0,
        0,
        GetModuleHandleW(null()),
        null(),
    );
    if hwnd == 0 {
        ShowWindow(parent, 5);
        return Err("无法打开选区编辑器".to_owned());
    }
    let state = Box::new(RefCell::new(State {
        roi: selected.roi,
        others,
        name,
        width: selected.width,
        height: selected.height,
        drag: None,
        accepted: false,
    }));
    SetWindowLongPtrW(hwnd, USER_DATA, (&*state as *const RefCell<State>) as isize);
    SetLayeredWindowAttributes(hwnd, 0, 170, 2);
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
    let state = state.borrow();
    if state.accepted {
        state.roi.validate(selected.width, selected.height)?;
        Ok(Some(Selection {
            roi: state.roi,
            ..selected
        }))
    } else {
        Ok(None)
    }
}
