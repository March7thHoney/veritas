#![allow(dead_code)]
use arboard::Clipboard;
use egui::{
    Context, Event, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, Theme,
    TouchId, Vec2, ViewportId,
};
use windows::{
    Wdk::System::SystemInformation::NtQuerySystemTime,
    Win32::{
        Foundation::{HWND, POINT, RECT},
        Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONEAREST, ScreenToClient},
        System::SystemServices::{MK_CONTROL, MK_SHIFT},
        UI::{
            Input::{
                KeyboardAndMouse::{
                    GetAsyncKeyState, VIRTUAL_KEY, VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END,
                    VK_ESCAPE, VK_HOME, VK_INSERT, VK_LEFT, VK_LSHIFT, VK_NEXT, VK_PRIOR,
                    VK_RETURN, VK_RIGHT, VK_SPACE, VK_TAB, VK_UP,
                },
                Pointer::{
                    GetPointerInfo, POINTER_BUTTON_CHANGE_TYPE, POINTER_FLAG_FIRSTBUTTON,
                    POINTER_FLAG_SECONDBUTTON, POINTER_INFO,
                },
            },
            Shell::GetScaleFactorForMonitor,
            WindowsAndMessaging::{
                GetClientRect, GetCursorPos, GetMessageExtraInfo, KF_REPEAT, PT_MOUSE, PT_TOUCH,
                WHEEL_DELTA, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN,
                WM_LBUTTONUP, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL,
                WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCMOUSEMOVE, WM_POINTERDOWN, WM_POINTERUP,
                WM_POINTERUPDATE, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN,
                WM_SYSKEYUP, WM_XBUTTONDBLCLK, WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1, XBUTTON2,
            },
        },
    },
};

pub struct InputHandler {
    pub ctx: Context,
    pub hwnd: HWND,
    pub events: Vec<Event>,
    pub modifiers: Option<Modifiers>,
    pub pointer_input_enabled: bool,
    relative_drag_enabled: bool,
    primary_button_down: bool,
    last_mouse_pos_px: Option<Pos2>,
    virtual_mouse_pos: Option<Pos2>,
    drag_warp_anchor_px: Option<Pos2>,
}

/// High-level overview of recognized `WndProc` messages.
#[repr(u8)]
pub enum InputResult {
    Unknown,
    MouseMove,
    MouseLeft,
    MouseRight,
    MouseMiddle,
    Character,
    Scroll,
    Zoom,
    Key,
}

impl InputResult {
    #[inline]
    pub fn is_valid(&self) -> bool {
        !self.is_unknown()
    }

    #[inline]
    pub fn is_unknown(&self) -> bool {
        matches!(*self, InputResult::Unknown)
    }
}

impl InputHandler {
    pub fn new(hwnd: HWND, ctx: &Context) -> Self {
        Self {
            ctx: ctx.clone(),
            hwnd,
            events: vec![],
            modifiers: None,
            pointer_input_enabled: Self::pointer_input_enabled(),
            relative_drag_enabled: Self::evernight_input_workaround_enabled(),
            primary_button_down: false,
            last_mouse_pos_px: None,
            virtual_mouse_pos: None,
            drag_warp_anchor_px: None,
        }
    }

    fn evernight_input_workaround_enabled() -> bool {
        std::env::var_os("EVERNIGHT_PATCH_UI_SCALE").is_some()
    }

    pub fn pointer_input_enabled() -> bool {
        !Self::evernight_input_workaround_enabled()
    }

    pub fn process(&mut self, umsg: u32, wparam: usize, lparam: isize) -> InputResult {
        match umsg {
            WM_POINTERUPDATE if self.pointer_input_enabled => {
                let mut pointer_info = POINTER_INFO::default();
                let pointer_id = wparam as u32 & 0xFFFF;
                unsafe {
                    GetPointerInfo(pointer_id, &mut pointer_info).unwrap();
                    let mut pt = pointer_info.ptPixelLocation;
                    ScreenToClient(self.hwnd, &mut pt).unwrap();

                    let pos = Pos2::new(pt.x as f32, pt.y as f32) / self.ctx.pixels_per_point();
                    self.events.push(Event::PointerMoved(pos));

                    if pointer_info.pointerType == PT_TOUCH {
                        self.events.push(Event::Touch {
                            device_id: egui::TouchDeviceId(0),
                            id: TouchId::from(pointer_id),
                            phase: egui::TouchPhase::Move,
                            pos,
                            force: None,
                        });
                    }
                    InputResult::MouseMove
                }
            }
            WM_POINTERDOWN | WM_POINTERUP if self.pointer_input_enabled => {
                let mut pointer_info = POINTER_INFO::default();
                let pointer_id = wparam as u32 & 0xFFFF;
                unsafe {
                    GetPointerInfo(pointer_id, &mut pointer_info).unwrap();
                    let mut pt = pointer_info.ptPixelLocation;

                    ScreenToClient(self.hwnd, &mut pt).unwrap();
                    let button = if pointer_info.pointerFlags.contains(POINTER_FLAG_FIRSTBUTTON) {
                        PointerButton::Primary
                    } else if pointer_info
                        .pointerFlags
                        .contains(POINTER_FLAG_SECONDBUTTON)
                    {
                        PointerButton::Secondary
                    } else {
                        PointerButton::Primary
                    };

                    let modifiers = if pointer_info.pointerType == PT_MOUSE {
                        let modifiers = get_mouse_modifiers(pointer_info.dwKeyStates as _);
                        self.alter_modifiers(modifiers);
                        modifiers
                    } else {
                        Modifiers::default()
                    };

                    let pressed = if umsg == WM_POINTERDOWN { true } else { false };
                    let pos = Pos2::new(pt.x as f32, pt.y as f32) / self.ctx.pixels_per_point();

                    self.events.push(Event::PointerButton {
                        pos,
                        button,
                        pressed,
                        modifiers,
                    });

                    if pointer_info.pointerType == PT_TOUCH {
                        if !pressed {
                            self.events.push(Event::PointerGone);
                        }

                        let phase = if pressed {
                            egui::TouchPhase::Start
                        } else {
                            egui::TouchPhase::End
                        };

                        self.events.push(Event::Touch {
                            device_id: egui::TouchDeviceId(0),
                            id: TouchId::from(pointer_id),
                            phase: phase,
                            pos,
                            force: None,
                        });
                    }
                    match button {
                        PointerButton::Primary => InputResult::MouseLeft,
                        PointerButton::Middle | PointerButton::Extra1 | PointerButton::Extra2 => {
                            InputResult::MouseMiddle
                        }
                        PointerButton::Secondary => InputResult::MouseRight,
                        _ => InputResult::Unknown,
                    }
                }
            }
            WM_POINTERUPDATE | WM_POINTERDOWN | WM_POINTERUP => {
                if self.relative_drag_enabled {
                    log::debug!(
                        "Evernight drag trace: ignored pointer message={umsg:#06x} wparam={wparam:#x} lparam={lparam:#x}"
                    );
                }
                InputResult::Unknown
            }
            WM_MOUSEMOVE | WM_NCMOUSEMOVE => {
                self.alter_modifiers(get_mouse_modifiers(wparam));

                if let Some(pos) = self.mouse_move_pos(lparam) {
                    self.events.push(Event::PointerMoved(pos));
                }
                InputResult::MouseMove
            }
            msg @ (WM_LBUTTONDOWN | WM_LBUTTONDBLCLK | WM_RBUTTONDOWN | WM_RBUTTONDBLCLK
            | WM_MBUTTONDOWN | WM_MBUTTONDBLCLK | WM_XBUTTONDOWN | WM_XBUTTONDBLCLK) => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                let (button, result) = match msg {
                    WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
                        (PointerButton::Primary, InputResult::MouseLeft)
                    }
                    WM_RBUTTONDOWN | WM_RBUTTONDBLCLK => {
                        (PointerButton::Secondary, InputResult::MouseRight)
                    }
                    WM_MBUTTONDOWN | WM_MBUTTONDBLCLK => {
                        (PointerButton::Middle, InputResult::MouseMiddle)
                    }
                    WM_XBUTTONDOWN | WM_XBUTTONDBLCLK => {
                        let button = if (wparam as u32) >> 16 & (XBUTTON1 as u32) != 0 {
                            PointerButton::Extra1
                        } else if (wparam as u32) >> 16 & (XBUTTON2 as u32) != 0 {
                            PointerButton::Extra2
                        } else {
                            unreachable!()
                        };
                        (button, InputResult::MouseMiddle)
                    }
                    _ => unreachable!(),
                };

                if button == PointerButton::Primary
                    && self.relative_drag_enabled
                    && self.primary_button_down
                {
                    if let Some(pos) =
                        self.update_relative_drag(Self::get_pos_px(lparam), true, "repeat-down")
                    {
                        self.events.push(Event::PointerMoved(pos));
                    }
                    return InputResult::MouseMove;
                }

                let pos = if button == PointerButton::Primary && self.relative_drag_enabled {
                    self.begin_relative_drag(lparam)
                } else {
                    self.get_pos(lparam)
                };

                self.events.push(Event::PointerButton {
                    pos,
                    button,
                    pressed: true,
                    modifiers,
                });

                result
            }

            msg @ (WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP | WM_XBUTTONUP) => {
                let modifiers = get_mouse_modifiers(wparam);
                self.alter_modifiers(modifiers);

                let (button, result) = match msg {
                    WM_LBUTTONUP => (PointerButton::Primary, InputResult::MouseLeft),
                    WM_RBUTTONUP => (PointerButton::Secondary, InputResult::MouseRight),
                    WM_MBUTTONUP => (PointerButton::Middle, InputResult::MouseMiddle),
                    WM_XBUTTONUP => {
                        let button = if (wparam as u32) >> 16 & (XBUTTON1 as u32) != 0 {
                            PointerButton::Extra1
                        } else if (wparam as u32) >> 16 & (XBUTTON2 as u32) != 0 {
                            PointerButton::Extra2
                        } else {
                            unreachable!()
                        };
                        (button, InputResult::MouseMiddle)
                    }
                    _ => unreachable!(),
                };

                let pos = if button == PointerButton::Primary && self.relative_drag_enabled {
                    if let Some(pos) =
                        self.update_relative_drag(Self::get_pos_px(lparam), true, "button-up")
                    {
                        self.events.push(Event::PointerMoved(pos));
                    }
                    self.end_relative_drag(lparam)
                } else {
                    self.get_pos(lparam)
                };

                self.events.push(Event::PointerButton {
                    pos,
                    button,
                    pressed: false,
                    modifiers,
                });

                result
            }
            WM_CHAR => {
                if let Some(ch) = char::from_u32(wparam as _) {
                    if !ch.is_control() {
                        self.events.push(Event::Text(ch.into()));
                    }
                }
                InputResult::Character
            }
            WM_MOUSEWHEEL => {
                self.alter_modifiers(get_mouse_modifiers(wparam));

                let delta = (wparam >> 16) as i16 as f32 * 10. / WHEEL_DELTA as f32;

                if wparam & MK_CONTROL.0 as usize != 0 {
                    self.events
                        .push(Event::Zoom(if delta > 0. { 1.5 } else { 0.5 }));
                    InputResult::Zoom
                } else {
                    self.events.push(Event::MouseWheel {
                        unit: MouseWheelUnit::Point,
                        delta: Vec2::new(0., delta),
                        modifiers: get_mouse_modifiers(wparam),
                    });
                    InputResult::Scroll
                }
            }
            WM_MOUSEHWHEEL => {
                self.alter_modifiers(get_mouse_modifiers(wparam));

                let delta = (wparam >> 16) as i16 as f32 * 10. / WHEEL_DELTA as f32;

                if wparam & MK_CONTROL.0 as usize != 0 {
                    self.events
                        .push(Event::Zoom(if delta > 0. { 1.5 } else { 0.5 }));
                    InputResult::Zoom
                } else {
                    self.events.push(Event::MouseWheel {
                        unit: MouseWheelUnit::Point,
                        delta: Vec2::new(delta, 0.),
                        modifiers: get_mouse_modifiers(wparam),
                    });
                    InputResult::Scroll
                }
            }
            msg @ (WM_KEYDOWN | WM_SYSKEYDOWN) => {
                let modifiers = get_key_modifiers(msg);
                self.modifiers = Some(modifiers);

                if let Some(key) = get_key(wparam) {
                    if key == Key::V && modifiers.ctrl {
                        if let Some(clipboard) = get_clipboard_text() {
                            self.events.push(Event::Text(clipboard));
                        }
                    }

                    if key == Key::C && modifiers.ctrl {
                        self.events.push(Event::Copy);
                    }

                    if key == Key::X && modifiers.ctrl {
                        self.events.push(Event::Cut);
                    }

                    self.events.push(Event::Key {
                        pressed: true,
                        physical_key: None,
                        modifiers,
                        key,
                        repeat: lparam & (KF_REPEAT as isize) > 0,
                    });
                }
                InputResult::Key
            }
            msg @ (WM_KEYUP | WM_SYSKEYUP) => {
                let modifiers = get_key_modifiers(msg);
                self.modifiers = Some(modifiers);

                if let Some(key) = get_key(wparam) {
                    self.events.push(Event::Key {
                        pressed: false,
                        physical_key: None,
                        modifiers,
                        key,
                        repeat: false,
                    });
                }
                InputResult::Key
            }
            _ => InputResult::Unknown,
        }
    }

    fn alter_modifiers(&mut self, new: Modifiers) {
        if let Some(old) = self.modifiers.as_mut() {
            *old = new;
        }
    }

    pub fn collect_input(&mut self, native_ppp: f32, effective_ppp: f32) -> RawInput {
        if let Some(pos) = self.poll_drag_position() {
            self.events.push(Event::PointerMoved(pos));
        }

        let mut rect = RECT::default();
        unsafe {
            GetClientRect(self.hwnd, &mut rect).unwrap();
        }
        let physical_size = Vec2::new(
            (rect.right - rect.left) as f32,
            (rect.bottom - rect.top) as f32,
        );

        let mut raw_input = RawInput {
            modifiers: self.modifiers.unwrap_or_default(),
            events: self.events.drain(..).collect::<Vec<Event>>(),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, physical_size / effective_ppp)),
            time: Some(Self::get_system_time()),
            system_theme: Self::get_system_theme(),
            focused: true,
            ..Default::default()
        };

        raw_input
            .viewports
            .entry(ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(native_ppp);

        raw_input
    }

    /// Returns time in seconds.
    pub fn get_system_time() -> f64 {
        let mut time = 0;
        unsafe {
            NtQuerySystemTime(&mut time).unwrap();
        }

        // dumb ass, read the docs. egui clearly says `in seconds`.
        // Shouldn't have wasted 3 days on this.
        // `NtQuerySystemTime` returns how many 100 nanosecond intervals
        // past since 1st Jan, 1601.
        (time as f64) / 10_000_000.
    }

    pub fn get_system_theme() -> Option<Theme> {
        match dark_light::detect() {
            Ok(mode) => match mode {
                dark_light::Mode::Dark => Some(Theme::Dark),
                dark_light::Mode::Light => Some(Theme::Light),
                dark_light::Mode::Unspecified => None,
            },
            Err(_) => None,
        }
    }

    #[inline]
    pub fn get_pixels_per_point(hwnd: HWND) -> f32 {
        let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };

        match unsafe { GetScaleFactorForMonitor(monitor) } {
            Ok(scale) if scale.0 > 0 => scale.0 as f32 / 100.0,
            Ok(_) => 1.0,
            Err(err) => {
                log::warn!(
                    "GetScaleFactorForMonitor failed: {:?}. Defaulting pixels_per_point to 1.0.",
                    err
                );
                1.0
            }
        }
    }

    #[inline]
    pub fn get_window_size(&self) -> Vec2 {
        let mut rect = RECT::default();
        unsafe {
            GetClientRect(self.hwnd, &mut rect).unwrap();
        }

        // Divide by scale
        Vec2::new(
            (rect.right - rect.left) as f32,
            (rect.bottom - rect.top) as f32,
        ) / self.ctx.pixels_per_point()
    }

    #[inline]
    pub fn get_window_rect(&self) -> Rect {
        Rect {
            min: Pos2::ZERO,
            max: self.get_window_size().to_pos2(),
        }
    }

    fn get_pos(&self, lparam: isize) -> Pos2 {
        Self::get_pos_px(lparam) / self.ctx.pixels_per_point()
    }

    fn get_pos_px(lparam: isize) -> Pos2 {
        let x = (lparam & 0xFFFF) as i16 as f32;
        let y = (lparam >> 16 & 0xFFFF) as i16 as f32;

        Pos2::new(x, y)
    }

    fn mouse_move_pos(&mut self, lparam: isize) -> Option<Pos2> {
        let current_px = Self::get_pos_px(lparam);
        if !self.relative_drag_enabled || !self.primary_button_down {
            let pos = current_px / self.ctx.pixels_per_point();
            self.last_mouse_pos_px = Some(current_px);
            self.virtual_mouse_pos = Some(pos);
            self.drag_warp_anchor_px = None;
            return Some(pos);
        }

        self.update_relative_drag(current_px, false, "message")
    }

    fn poll_drag_position(&mut self) -> Option<Pos2> {
        if !self.relative_drag_enabled || !self.primary_button_down {
            return None;
        }

        let mut point = POINT::default();
        if unsafe { GetCursorPos(&mut point) }.is_err()
            || !unsafe { ScreenToClient(self.hwnd, &mut point) }.as_bool()
        {
            return None;
        }

        self.update_relative_drag(
            Pos2::new(point.x as f32, point.y as f32),
            false,
            "frame-poll",
        )
    }

    fn update_relative_drag(
        &mut self,
        current_px: Pos2,
        accept_large_delta: bool,
        source: &str,
    ) -> Option<Pos2> {
        let previous_px = self.last_mouse_pos_px.replace(current_px)?;
        let pixels_per_point = self.ctx.pixels_per_point().max(f32::EPSILON);
        let delta_px = current_px - previous_px;
        if delta_px.x.abs() < 0.5 && delta_px.y.abs() < 0.5 {
            return None;
        }
        let tolerance_px = 2.0 * pixels_per_point;

        if let Some(anchor_px) = self.drag_warp_anchor_px {
            let anchor_delta = current_px - anchor_px;
            if anchor_delta.x.abs() <= tolerance_px && anchor_delta.y.abs() <= tolerance_px {
                log::debug!(
                    "Evernight drag trace: anchor source={} raw=({:.1},{:.1}) delta=({:.1},{:.1}) ppp={:.3}",
                    source,
                    current_px.x,
                    current_px.y,
                    delta_px.x,
                    delta_px.y,
                    pixels_per_point
                );
                return None;
            }
        }

        let jump_threshold_px = 96.0 * pixels_per_point;
        if !accept_large_delta
            && (delta_px.x.abs() > jump_threshold_px || delta_px.y.abs() > jump_threshold_px)
        {
            self.drag_warp_anchor_px = Some(current_px);
            log::debug!(
                "Evernight drag trace: jump source={} raw=({:.1},{:.1}) delta=({:.1},{:.1}) ppp={:.3}",
                source,
                current_px.x,
                current_px.y,
                delta_px.x,
                delta_px.y,
                pixels_per_point
            );
            return None;
        }

        let pos = self.virtual_mouse_pos.unwrap_or(current_px / pixels_per_point)
            + delta_px / pixels_per_point;
        self.virtual_mouse_pos = Some(pos);
        log::debug!(
            "Evernight drag trace: move source={} raw=({:.1},{:.1}) delta=({:.1},{:.1}) virtual=({:.1},{:.1}) ppp={:.3}",
            source,
            current_px.x,
            current_px.y,
            delta_px.x,
            delta_px.y,
            pos.x,
            pos.y,
            pixels_per_point
        );
        Some(pos)
    }

    fn begin_relative_drag(&mut self, lparam: isize) -> Pos2 {
        let pos_px = Self::get_pos_px(lparam);
        let pos = pos_px / self.ctx.pixels_per_point();
        self.primary_button_down = true;
        self.last_mouse_pos_px = Some(pos_px);
        self.virtual_mouse_pos = Some(pos);
        self.drag_warp_anchor_px = Some(pos_px);
        log::info!(
            "Evernight drag trace: begin raw=({:.1},{:.1}) logical=({:.1},{:.1}) ppp={:.3}",
            pos_px.x,
            pos_px.y,
            pos.x,
            pos.y,
            self.ctx.pixels_per_point()
        );
        pos
    }

    fn end_relative_drag(&mut self, lparam: isize) -> Pos2 {
        let pos = self.virtual_mouse_pos.unwrap_or_else(|| self.get_pos(lparam));
        self.primary_button_down = false;
        self.last_mouse_pos_px = Some(Self::get_pos_px(lparam));
        self.virtual_mouse_pos = Some(pos);
        self.drag_warp_anchor_px = None;
        log::info!(
            "Evernight drag trace: end raw=({:.1},{:.1}) logical=({:.1},{:.1}) ppp={:.3}",
            Self::get_pos_px(lparam).x,
            Self::get_pos_px(lparam).y,
            pos.x,
            pos.y,
            self.ctx.pixels_per_point()
        );
        pos
    }
}

fn get_mouse_modifiers(wparam: usize) -> Modifiers {
    Modifiers {
        alt: false,
        ctrl: (wparam & MK_CONTROL.0 as usize) != 0,
        shift: (wparam & MK_SHIFT.0 as usize) != 0,
        mac_cmd: false,
        command: (wparam & MK_CONTROL.0 as usize) != 0,
    }
}

fn get_key_modifiers(msg: u32) -> Modifiers {
    let ctrl = unsafe { GetAsyncKeyState(VK_CONTROL.0 as _) != 0 };
    let shift = unsafe { GetAsyncKeyState(VK_LSHIFT.0 as _) != 0 };

    Modifiers {
        alt: msg == WM_SYSKEYDOWN,
        mac_cmd: false,
        command: ctrl,
        shift,
        ctrl,
    }
}

fn get_key(wparam: usize) -> Option<Key> {
    match wparam {
        0x08 => Some(Key::Backspace),
        0x09 => Some(Key::Tab),
        0x0D => Some(Key::Enter),
        0x1B => Some(Key::Escape),
        0x20 => Some(Key::Space),
        0x21 => Some(Key::PageUp),
        0x22 => Some(Key::PageDown),
        0x23 => Some(Key::End),
        0x24 => Some(Key::Home),
        0x25 => Some(Key::ArrowLeft),
        0x26 => Some(Key::ArrowUp),
        0x27 => Some(Key::ArrowRight),
        0x28 => Some(Key::ArrowDown),
        0x2D => Some(Key::Insert),
        0x2E => Some(Key::Delete),
        0x30 => Some(Key::Num0),
        0x31 => Some(Key::Num1),
        0x32 => Some(Key::Num2),
        0x33 => Some(Key::Num3),
        0x34 => Some(Key::Num4),
        0x35 => Some(Key::Num5),
        0x36 => Some(Key::Num6),
        0x37 => Some(Key::Num7),
        0x38 => Some(Key::Num8),
        0x39 => Some(Key::Num9),
        0x41 => Some(Key::A),
        0x42 => Some(Key::B),
        0x43 => Some(Key::C),
        0x44 => Some(Key::D),
        0x45 => Some(Key::E),
        0x46 => Some(Key::F),
        0x47 => Some(Key::G),
        0x48 => Some(Key::H),
        0x49 => Some(Key::I),
        0x4A => Some(Key::J),
        0x4B => Some(Key::K),
        0x4C => Some(Key::L),
        0x4D => Some(Key::M),
        0x4E => Some(Key::N),
        0x4F => Some(Key::O),
        0x50 => Some(Key::P),
        0x51 => Some(Key::Q),
        0x52 => Some(Key::R),
        0x53 => Some(Key::S),
        0x54 => Some(Key::T),
        0x55 => Some(Key::U),
        0x56 => Some(Key::V),
        0x57 => Some(Key::W),
        0x58 => Some(Key::X),
        0x59 => Some(Key::Y),
        0x5A => Some(Key::Z),
        0x70 => Some(Key::F1),
        0x71 => Some(Key::F2),
        0x72 => Some(Key::F3),
        0x73 => Some(Key::F4),
        0x74 => Some(Key::F5),
        0x75 => Some(Key::F6),
        0x76 => Some(Key::F7),
        0x77 => Some(Key::F8),
        0x78 => Some(Key::F9),
        0x79 => Some(Key::F10),
        0x7A => Some(Key::F11),
        0x7B => Some(Key::F12),
        0x7C => Some(Key::F13),
        0x7D => Some(Key::F14),
        0x7E => Some(Key::F15),
        0x7F => Some(Key::F16),
        0x80 => Some(Key::F17),
        0x81 => Some(Key::F18),
        0x82 => Some(Key::F19),
        0x83 => Some(Key::F20),
        0x84 => Some(Key::F21),
        0x85 => Some(Key::F22),
        0x86 => Some(Key::F23),
        0x87 => Some(Key::F24),
        0xBA => Some(Key::Semicolon),
        0xBB => Some(Key::Equals),
        0xBC => Some(Key::Comma),
        0xBD => Some(Key::Minus),
        0xBE => Some(Key::Period),
        0xBF => Some(Key::Slash),
        0xC0 => Some(Key::Backtick),
        0xDB => Some(Key::OpenBracket),
        0xDC => Some(Key::Backslash),
        0xDD => Some(Key::CloseBracket),
        0xDE => Some(Key::Quote),
        _ => None,
    }
}

fn get_clipboard_text() -> Option<String> {
    let mut clipboard = Clipboard::new().unwrap();
    clipboard.get_text().ok()
}
