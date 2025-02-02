use baseview::{
    Event, EventStatus, Window, WindowHandle, WindowHandler, WindowOpenOptions,
    MouseCursor,
};
use copypasta::ClipboardProvider;
use egui::{pos2, vec2, Pos2, Rect, Rgba, CursorIcon, ClipboardMime, ClipboardData};
use keyboard_types::Modifiers;
use raw_window_handle::HasRawWindowHandle;
use std::time::Instant;

use crate::renderer::Renderer;

pub struct Queue<'a> {
    bg_color: &'a mut Rgba,
    close_requested: &'a mut bool,
}

impl<'a> Queue<'a> {
    pub(crate) fn new(bg_color: &'a mut Rgba, close_requested: &'a mut bool) -> Self {
        Self {
            bg_color,
            //renderer,
            //repaint_requested,
            close_requested,
        }
    }

    /// Set the background color.
    pub fn bg_color(&mut self, bg_color: Rgba) {
        *self.bg_color = bg_color;
    }

    /// Close the window.
    pub fn close_window(&mut self) {
        *self.close_requested = true;
    }
}

struct OpenSettings {
    pub physical_width: f64,
    pub physical_height: f64,
}

impl OpenSettings {
    fn new(settings: &WindowOpenOptions) -> Self {
        Self {
            physical_width: settings.size.width as f64,
            physical_height: settings.size.height as f64,
        }
    }
}

/// Handles an egui-baseview application
pub struct EguiWindow<State, U>
where
    State: 'static + Send,
    U: FnMut(&egui::Context, &mut Queue, &mut State),
    U: 'static + Send,
{
    user_state: Option<State>,
    user_update: U,

    egui_ctx: egui::Context,
    egui_input: egui::RawInput,
    clipboard_ctx: Option<copypasta::ClipboardContext>,

    renderer: Renderer,
    scale_factor: f32,
    bg_color: Rgba,
    physical_width: u32,
    physical_height: u32,
    start_time: Instant,
    repaint_after: Option<Instant>,
    mouse_pos: Option<Pos2>,
    close_requested: bool,
    last_cursor_icon: Option<CursorIcon>,
}

impl<State, U> EguiWindow<State, U>
where
    State: 'static + Send,
    U: FnMut(&egui::Context, &mut Queue, &mut State),
    U: 'static + Send,
{
    fn new<B>(
        window: &mut baseview::Window<'_>,
        open_settings: OpenSettings,
        mut build: B,
        update: U,
        mut state: State,
    ) -> EguiWindow<State, U>
    where
        B: FnMut(&egui::Context, &mut Queue, &mut State),
        B: 'static + Send,
    {
        let egui_ctx = egui::Context::default();

        let guessed_scale = 1.0; // This is a wild guess. After we received some message, we'll know better.

        let egui_input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::new(0f32, 0f32),
                vec2(
                    open_settings.physical_width as f32,
                    open_settings.physical_height as f32,
                ),
            )),
            pixels_per_point: Some(guessed_scale),
            modifiers: egui::Modifiers {
                alt: false,
                ctrl: false,
                shift: false,
                mac_cmd: false,
                command: false,
            },
            ..Default::default()
        };

        let physical_width = open_settings.physical_width.round() as u32;
        let physical_height = open_settings.physical_height.round() as u32;

        let renderer = Renderer::new(window);

        let mut bg_color = Rgba::BLACK;
        let mut close_requested = false;
        let mut queue = Queue::new(
            &mut bg_color,
            //&mut renderer,
            //&mut repaint_requested,
            &mut close_requested,
        );
        (build)(&egui_ctx, &mut queue, &mut state);

        let clipboard_ctx = match copypasta::ClipboardContext::new() {
            Ok(clipboard_ctx) => Some(clipboard_ctx),
            Err(e) => {
                eprintln!("Failed to initialize clipboard: {}", e);
                None
            }
        };

        Self {
            user_state: Some(state),
            user_update: update,

            egui_ctx,
            egui_input,
            clipboard_ctx,

            renderer,
            scale_factor: guessed_scale,
            bg_color,
            physical_width,
            physical_height,
            start_time: Instant::now(),
            repaint_after: Some(Instant::now()),
            mouse_pos: None,
            close_requested,
            last_cursor_icon: None,
        }
    }

    /// Open a new child window.
    ///
    /// * `parent` - The parent window.
    /// * `settings` - The settings of the window.
    /// * `state` - The initial state of your application.
    /// * `build` - Called once before the first frame. Allows you to do setup code and to
    /// call `ctx.set_fonts()`. Optional.
    /// * `update` - Called before each frame. Here you should update the state of your
    /// application and build the UI.
    pub fn open_parented<P, B>(
        parent: &P,
        mut settings: WindowOpenOptions,
        state: State,
        build: B,
        update: U,
    ) -> WindowHandle
    where
        P: HasRawWindowHandle,
        B: FnMut(&egui::Context, &mut Queue, &mut State),
        B: 'static + Send,
    {
        if settings.gl_config.is_none() {
            settings.gl_config = Some(Default::default());
        }

        let open_settings = OpenSettings::new(&settings);

        Window::open_parented(
            parent,
            settings,
            move |window: &mut baseview::Window<'_>| -> EguiWindow<State, U> {
                EguiWindow::new(window, open_settings, build, update, state)
            },
        )
    }

    /// Open a new window as if it had a parent window.
    ///
    /// * `settings` - The settings of the window.
    /// * `state` - The initial state of your application.
    /// * `build` - Called once before the first frame. Allows you to do setup code and to
    /// call `ctx.set_fonts()`. Optional.
    /// * `update` - Called before each frame. Here you should update the state of your
    /// application and build the UI.
    pub fn open_as_if_parented<B>(
        mut settings: WindowOpenOptions,
        state: State,
        build: B,
        update: U,
    ) -> WindowHandle
    where
        B: FnMut(&egui::Context, &mut Queue, &mut State),
        B: 'static + Send,
    {
        if settings.gl_config.is_none() {
            settings.gl_config = Some(Default::default());
        }

        let open_settings = OpenSettings::new(&settings);

        Window::open_as_if_parented(
            settings,
            move |window: &mut baseview::Window<'_>| -> EguiWindow<State, U> {
                EguiWindow::new(window, open_settings, build, update, state)
            },
        )
    }

    /// Open a new window that blocks the current thread until the window is destroyed.
    ///
    /// * `settings` - The settings of the window.
    /// * `state` - The initial state of your application.
    /// * `build` - Called once before the first frame. Allows you to do setup code and to
    /// call `ctx.set_fonts()`. Optional.
    /// * `update` - Called before each frame. Here you should update the state of your
    /// application and build the UI.
    pub fn open_blocking<B>(mut settings: WindowOpenOptions, state: State, build: B, update: U)
    where
        B: FnMut(&egui::Context, &mut Queue, &mut State),
        B: 'static + Send,
    {
        if settings.gl_config.is_none() {
            settings.gl_config = Some(Default::default());
        }

        let open_settings = OpenSettings::new(&settings);

        Window::open_blocking(
            settings,
            move |window: &mut baseview::Window<'_>| -> EguiWindow<State, U> {
                EguiWindow::new(window, open_settings, build, update, state)
            },
        )
    }

    /// Update the pressed key modifiers when a mouse event has sent a new set of modifiers.
    fn update_modifiers(&mut self, modifiers: &Modifiers) {
        self.egui_input.modifiers.alt = !(*modifiers & Modifiers::ALT).is_empty();
        self.egui_input.modifiers.shift = !(*modifiers & Modifiers::SHIFT).is_empty();
        self.egui_input.modifiers.command = !(*modifiers & Modifiers::CONTROL).is_empty();
    }
}

impl<State, U> WindowHandler for EguiWindow<State, U>
where
    State: 'static + Send,
    U: FnMut(&egui::Context, &mut Queue, &mut State),
    U: 'static + Send,
{
    fn on_frame(&mut self, window: &mut Window) {
        if let Some(state) = &mut self.user_state {
            self.egui_input.time = Some(self.start_time.elapsed().as_nanos() as f64 * 1e-9);
            self.egui_ctx.begin_frame(self.egui_input.take());

            //let mut repaint_requested = false;
            let mut queue = Queue::new(
                &mut self.bg_color,
                //&mut self.renderer,
                //&mut repaint_requested,
                &mut self.close_requested,
            );

            (self.user_update)(&self.egui_ctx, &mut queue, state);

            let egui::FullOutput {
                platform_output,
                repaint_after,
                mut textures_delta,
                mut shapes,
            } = self.egui_ctx.end_frame();

            let now = Instant::now();
            let do_repaint_now = if let Some(t) = self.repaint_after {
                now >= t || repaint_after.is_zero()
            } else {
                repaint_after.is_zero()
            };

            if do_repaint_now {
                self.renderer.render(
                    window,
                    self.bg_color,
                    self.physical_width,
                    self.physical_height,
                    self.scale_factor,
                    &mut self.egui_ctx,
                    &mut shapes,
                    &mut textures_delta,
                );

                self.repaint_after = None;
            } else if let Some(repaint_after) = now.checked_add(repaint_after) {
                // Schedule to repaint after the requested time has elapsed.
                self.repaint_after = Some(repaint_after);
            }

            if !platform_output.copied_text.is_empty() {
                if let Some(clipboard_ctx) = &mut self.clipboard_ctx {
                    if let Err(err) = clipboard_ctx.set_contents(platform_output.copied_text) {
                        eprintln!("Copy/Cut error: {}", err);
                    }
                }
            }

            if let Some(egui::ClipboardData { data, mime: ClipboardMime::Specific(mime)}) = platform_output.copied_data {
                if let Some(clipboard_ctx) = &mut self.clipboard_ctx {
                    if let Err(err) = clipboard_ctx.set_mime_contents(data, &mime) {
                        eprintln!("Copy/Cut error: {}", err);
                    }
                }
            }

            // set the cursor icon
            if self.last_cursor_icon != Some(platform_output.cursor_icon) {
                // CAUTION: Setting the same cursor icon every frame causes signifigant lag in MacOS 
                //   -> so we only set the cursor if it changed.
                self.last_cursor_icon = Some(platform_output.cursor_icon);
                window.set_mouse_cursor(translate_cursor_icon(platform_output.cursor_icon));
            }

            if self.close_requested {
                window.close();
            }
        }
    }

    fn on_event(&mut self, _window: &mut Window, event: Event) -> EventStatus {
        match &event {
            baseview::Event::Mouse(event) => match event {
                baseview::MouseEvent::CursorMoved {
                    position,
                    modifiers,
                } => {
                    self.update_modifiers(modifiers);

                    let pos = pos2(position.x as f32, position.y as f32);
                    self.mouse_pos = Some(pos);
                    self.egui_input.events.push(egui::Event::PointerMoved(pos));
                }
                baseview::MouseEvent::ButtonPressed { button, modifiers } => {
                    self.update_modifiers(modifiers);

                    if let Some(pos) = self.mouse_pos {
                        if let Some(button) = translate_mouse_button(*button) {
                            self.egui_input.events.push(egui::Event::PointerButton {
                                pos,
                                button,
                                pressed: true,
                                modifiers: self.egui_input.modifiers,
                            });
                        }
                    }
                }
                baseview::MouseEvent::ButtonReleased { button, modifiers } => {
                    self.update_modifiers(modifiers);

                    if let Some(pos) = self.mouse_pos {
                        if let Some(button) = translate_mouse_button(*button) {
                            self.egui_input.events.push(egui::Event::PointerButton {
                                pos,
                                button,
                                pressed: false,
                                modifiers: self.egui_input.modifiers,
                            });
                        }
                    }
                }
                baseview::MouseEvent::WheelScrolled {
                    delta: scroll_delta,
                    modifiers,
                } => {
                    self.update_modifiers(modifiers);

                    let delta = match scroll_delta {
                        baseview::ScrollDelta::Lines { x, y } => {
                            let points_per_scroll_line = 50.0; // Scroll speed decided by consensus: https://github.com/emilk/egui/issues/461
                            egui::vec2(*x, *y) * points_per_scroll_line
                        }
                        baseview::ScrollDelta::Pixels { x, y } => {
                            if let Some(pixels_per_point) = self.egui_input.pixels_per_point {
                                egui::vec2(*x, *y) / pixels_per_point
                            } else {
                                egui::vec2(*x, *y)
                            }
                        }
                    };

                    if self.egui_input.modifiers.ctrl || self.egui_input.modifiers.command {
                        // Treat as zoom instead:
                        let factor = (delta.y / 200.0).exp();
                        self.egui_input.events.push(egui::Event::Zoom(factor));
                    } else if self.egui_input.modifiers.shift {
                        // Treat as horizontal scrolling.
                        // Note: one Mac we already get horizontal scroll events when shift is down.
                        self.egui_input
                            .events
                            .push(egui::Event::Scroll(egui::vec2(delta.x + delta.y, 0.0)));
                    } else {
                        self.egui_input.events.push(egui::Event::Scroll(delta));
                    }
                }
                baseview::MouseEvent::CursorLeft => {
                    self.mouse_pos = None;
                    self.egui_input.events.push(egui::Event::PointerGone);
                }
                _ => {}
            },
            baseview::Event::Keyboard(event) => {
                EguiKeyboardInput::from_keyboard_event(&event, self.clipboard_ctx.as_mut())
                    .apply_on_raw_input(&mut self.egui_input);
            }
            baseview::Event::Window(event) => match event {
                baseview::WindowEvent::Resized(window_info) => {
                    self.scale_factor = window_info.scale() as f32;

                    let logical_size = (
                        (window_info.physical_size().width as f32 / self.scale_factor),
                        (window_info.physical_size().height as f32 / self.scale_factor),
                    );

                    self.physical_width = window_info.physical_size().width;
                    self.physical_height = window_info.physical_size().height;

                    self.egui_input.pixels_per_point = Some(self.scale_factor);

                    self.egui_input.screen_rect = Some(Rect::from_min_size(
                        Pos2::new(0f32, 0f32),
                        vec2(logical_size.0, logical_size.1),
                    ));

                    // Schedule to repaint on the next frame.
                    self.repaint_after = Some(Instant::now());
                }
                baseview::WindowEvent::WillClose => {}
                _ => {}
            },
        }

        EventStatus::Captured
    }
}

pub struct EguiKeyboardInput {
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
}
impl EguiKeyboardInput {
    pub fn from_keyboard_event(event: &keyboard_types::KeyboardEvent, clipboard_ctx: Option<&mut copypasta::ClipboardContext>) -> EguiKeyboardInput {
        let mut events = vec![];
        let mut modifiers = translate_modifiers(&event.modifiers);

        use keyboard_types::Code;

        let pressed = event.state == keyboard_types::KeyState::Down;

        match event.code {
            Code::ShiftLeft | Code::ShiftRight => modifiers.shift = pressed,
            Code::ControlLeft | Code::ControlRight => {
                modifiers.ctrl = pressed;

                #[cfg(not(target_os = "macos"))]
                {
                    modifiers.command = pressed;
                }
            }
            Code::AltLeft | Code::AltRight => modifiers.alt = pressed,
            Code::MetaLeft | Code::MetaRight => {
                #[cfg(target_os = "macos")]
                {
                    modifiers.mac_cmd = pressed;
                    modifiers.command = pressed;
                }
                () // prevent `rustfmt` from breaking this
            }
            _ => (),
        }

        if let Some(key) = translate_virtual_key_code(event.code) {
            events.push(egui::Event::Key { key, pressed, modifiers });
        }

        if pressed {
            // VirtualKeyCode::Paste etc in winit are broken/untrustworthy,
            // so we detect these things manually:
            if is_cut_command(modifiers, event.code) {
                events.push(egui::Event::Cut);
            } else if is_copy_command(modifiers, event.code) {
                events.push(egui::Event::Copy);
            } else if is_paste_command(modifiers, event.code) {
                if let Some(clipboard_ctx) = clipboard_ctx {
                    if let Ok(contents) = clipboard_ctx.get_contents() {
                        events.push(egui::Event::Paste(contents));
                    }
                    if let Ok(data) = clipboard_ctx.get_mime_contents("application/dspstudio") {
                        events.push(
                            egui::Event::PasteMime(ClipboardData {
                                data,
                                mime: ClipboardMime::Specific("application/dspstudio".to_string())
                            })
                        );
                    }
                }
            } else if let keyboard_types::Key::Character(written) = &event.key {
                if !modifiers.ctrl && !modifiers.command {
                    events.push(egui::Event::Text(written.clone()));
                }
            }
        }
        EguiKeyboardInput {
            events,
            modifiers
        }
    }

    pub fn apply_on_input(self, input_mut: &mut egui::InputState) {
        for event in self.events {
            if let egui::Event::Key { key, pressed, .. } = &event {
                if *pressed {
                    input_mut.keys_down.insert(*key);
                } else {
                    input_mut.keys_down.remove(key);
                }
            }
            input_mut.raw.events.push(event.clone());
            input_mut.events.push(event);
        }
        input_mut.raw.modifiers = self.modifiers;
        input_mut.modifiers = self.modifiers;
    }

    pub fn apply_on_raw_input(self, raw_input_mut: &mut egui::RawInput) {
        for event in self.events {
            raw_input_mut.events.push(event.clone());
        }
        raw_input_mut.modifiers = self.modifiers;
    }
}

pub fn translate_mouse_button(button: baseview::MouseButton) -> Option<egui::PointerButton> {
    match button {
        baseview::MouseButton::Left => Some(egui::PointerButton::Primary),
        baseview::MouseButton::Right => Some(egui::PointerButton::Secondary),
        baseview::MouseButton::Middle => Some(egui::PointerButton::Middle),
        _ => None,
    }
}

pub fn translate_virtual_key_code(key: keyboard_types::Code) -> Option<egui::Key> {
    use egui::Key;
    use keyboard_types::Code;

    Some(match key {
        Code::ArrowDown => Key::ArrowDown,
        Code::ArrowLeft => Key::ArrowLeft,
        Code::ArrowRight => Key::ArrowRight,
        Code::ArrowUp => Key::ArrowUp,

        Code::Escape => Key::Escape,
        Code::Tab => Key::Tab,
        Code::Backspace => Key::Backspace,
        Code::Enter | Code::NumpadEnter => Key::Enter,
        Code::Space => Key::Space,

        Code::Insert => Key::Insert,
        Code::Delete => Key::Delete,
        Code::Home => Key::Home,
        Code::End => Key::End,
        Code::PageUp => Key::PageUp,
        Code::PageDown => Key::PageDown,

        Code::Digit0 | Code::Numpad0 => Key::Num0,
        Code::Digit1 | Code::Numpad1 => Key::Num1,
        Code::Digit2 | Code::Numpad2 => Key::Num2,
        Code::Digit3 | Code::Numpad3 => Key::Num3,
        Code::Digit4 | Code::Numpad4 => Key::Num4,
        Code::Digit5 | Code::Numpad5 => Key::Num5,
        Code::Digit6 | Code::Numpad6 => Key::Num6,
        Code::Digit7 | Code::Numpad7 => Key::Num7,
        Code::Digit8 | Code::Numpad8 => Key::Num8,
        Code::Digit9 | Code::Numpad9 => Key::Num9,

        Code::KeyA => Key::A,
        Code::KeyB => Key::B,
        Code::KeyC => Key::C,
        Code::KeyD => Key::D,
        Code::KeyE => Key::E,
        Code::KeyF => Key::F,
        Code::KeyG => Key::G,
        Code::KeyH => Key::H,
        Code::KeyI => Key::I,
        Code::KeyJ => Key::J,
        Code::KeyK => Key::K,
        Code::KeyL => Key::L,
        Code::KeyM => Key::M,
        Code::KeyN => Key::N,
        Code::KeyO => Key::O,
        Code::KeyP => Key::P,
        Code::KeyQ => Key::Q,
        Code::KeyR => Key::R,
        Code::KeyS => Key::S,
        Code::KeyT => Key::T,
        Code::KeyU => Key::U,
        Code::KeyV => Key::V,
        Code::KeyW => Key::W,
        Code::KeyX => Key::X,
        Code::KeyY => Key::Y,
        Code::KeyZ => Key::Z,
        _ => {
            return None;
        }
    })
}

pub fn is_cut_command(modifiers: egui::Modifiers, keycode: keyboard_types::Code) -> bool {
    (modifiers.command && keycode == keyboard_types::Code::KeyX)
        || (cfg!(target_os = "windows")
            && modifiers.shift
            && keycode == keyboard_types::Code::Delete)
}

pub fn is_copy_command(modifiers: egui::Modifiers, keycode: keyboard_types::Code) -> bool {
    (modifiers.command && keycode == keyboard_types::Code::KeyC)
        || (cfg!(target_os = "windows")
            && modifiers.ctrl
            && keycode == keyboard_types::Code::Insert)
}

pub fn is_paste_command(modifiers: egui::Modifiers, keycode: keyboard_types::Code) -> bool {
    (modifiers.command && keycode == keyboard_types::Code::KeyV)
        || (cfg!(target_os = "windows")
            && modifiers.shift
            && keycode == keyboard_types::Code::Insert)
}


fn translate_cursor_icon(icon: CursorIcon) -> MouseCursor {
    match icon {
        CursorIcon::Default => MouseCursor::Default,
        CursorIcon::None => MouseCursor::Hidden,
        CursorIcon::ContextMenu => MouseCursor::Default,
        CursorIcon::Help => MouseCursor::Help,
        CursorIcon::PointingHand => MouseCursor::Pointer,
        CursorIcon::Progress=> MouseCursor::PtrWorking,
        CursorIcon::Wait => MouseCursor::Working,
        CursorIcon::Cell => MouseCursor::Cell,
        CursorIcon::Crosshair => MouseCursor::Crosshair,
        CursorIcon::Text => MouseCursor::Text,
        CursorIcon::VerticalText => MouseCursor::VerticalText,
        CursorIcon::Alias => MouseCursor::Alias,
        CursorIcon::Copy => MouseCursor::Copy,
        CursorIcon::Move => MouseCursor::Move,
        CursorIcon::NoDrop => MouseCursor::PtrNotAllowed,
        CursorIcon::NotAllowed => MouseCursor::NotAllowed,
        CursorIcon::Grab => MouseCursor::Hand,
        CursorIcon::Grabbing => MouseCursor::HandGrabbing,
        CursorIcon::AllScroll => MouseCursor::AllScroll,
        CursorIcon::ResizeHorizontal => MouseCursor::EwResize,
        CursorIcon::ResizeNeSw => MouseCursor::NeswResize,
        CursorIcon::ResizeNwSe => MouseCursor::NwseResize,
        CursorIcon::ResizeVertical => MouseCursor::NsResize,
        CursorIcon::ResizeEast => MouseCursor::EResize,
        CursorIcon::ResizeSouthEast => MouseCursor::SeResize,
        CursorIcon::ResizeSouth => MouseCursor::SResize,
        CursorIcon::ResizeSouthWest => MouseCursor::SwResize,
        CursorIcon::ResizeWest => MouseCursor::WResize,
        CursorIcon::ResizeNorthWest => MouseCursor::NwResize,
        CursorIcon::ResizeNorth => MouseCursor::NResize,
        CursorIcon::ResizeNorthEast => MouseCursor::NeResize,
        CursorIcon::ResizeColumn => MouseCursor::ColResize,
        CursorIcon::ResizeRow => MouseCursor::RowResize,
        CursorIcon::ZoomIn => MouseCursor::ZoomIn,
        CursorIcon::ZoomOut => MouseCursor::ZoomOut,
    }
}


pub fn translate_modifiers(modifiers: &keyboard_types::Modifiers) -> egui::Modifiers {
    egui::Modifiers {
        alt: modifiers.contains(keyboard_types::Modifiers::ALT),
        command: modifiers.contains(keyboard_types::Modifiers::META) || (!cfg!(target_os = "macos") && modifiers.contains(keyboard_types::Modifiers::CONTROL)),
        ctrl: modifiers.contains(keyboard_types::Modifiers::CONTROL) || (!cfg!(target_os = "macos") && modifiers.contains(keyboard_types::Modifiers::META)),
        mac_cmd: cfg!(target_os = "macos") && modifiers.contains(keyboard_types::Modifiers::META),
        shift: modifiers.contains(keyboard_types::Modifiers::SHIFT),
    }
}
