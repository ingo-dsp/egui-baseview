use baseview::Window;
use egui_glow::Painter;
use std::sync::Arc;

use std::sync::Mutex;
use egui::{Id, Rgba};
use egui_glow::glow;
use std::ops::Deref;
use std::borrow::BorrowMut;


pub struct Renderer {
    glow_context: Arc<egui_glow::glow::Context>,
    painter: Painter,
    id_renderer: Id,
}

impl Renderer {
    pub fn new(window: &Window) -> Self {
        let context = window
            .gl_context()
            .expect("failed to get baseview gl context");
        unsafe {
            context.make_current();
        }

        let glow_context = Arc::new(unsafe {
            egui_glow::glow::Context::from_loader_function(|s| context.get_proc_address(s))
        });

        let painter = egui_glow::Painter::new(Arc::clone(&glow_context), None, "")
            .map_err(|error| {
                eprintln!("error occurred in initializing painter:\n{}", error);
            })
            .unwrap();

        unsafe {
            context.make_not_current();
        }

        Self {
            glow_context,
            painter,
            id_renderer: Id::new("dspstudio-renderer"),
        }
    }

    pub fn render(
        &mut self,
        window: &Window,
        bg_color: egui::Rgba,
        canvas_width: u32,
        canvas_height: u32,
        pixels_per_point: f32,
        egui_ctx: &mut egui::Context,
        shapes: &mut Vec<egui::epaint::ClippedShape>,
        textures_delta: &mut egui::TexturesDelta,
    ) {
        let shapes = std::mem::take(shapes);
        let mut textures_delta = std::mem::take(textures_delta);

        let context = window
            .gl_context()
            .expect("failed to get baseview gl context");
        unsafe {
            context.make_current();
        }

        // BEGIN MODIFIED
        let gl = &self.glow_context;
        // NOTE: We need to clear in sRGB on MacOS, so for simplicity we do that for every platform.
        let color = Rgba::from_srgba_premultiplied(32, 32, 32, 255); // This happens to be premultiplied already because alpha is 255.
        unsafe {
            use egui_glow::glow::HasContext as _;
            gl.enable(glow::FRAMEBUFFER_SRGB);
            gl.disable(glow::SCISSOR_TEST);
            gl.clear_color(color[0], color[1], color[2], color[3]);
            gl.clear_depth_f32(1.0);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            gl.disable(glow::FRAMEBUFFER_SRGB);

            let additional_renderer: Option<Arc<Mutex<Box<(dyn Fn(&glow::Context) + Send + Sync)>>>> = egui_ctx.memory().data.get_temp(self.id_renderer);
            if let Some(additional_renderer) = additional_renderer {
                if let Ok(mut renderer) = additional_renderer.try_lock() {
                    gl.disable(glow::FRAMEBUFFER_SRGB);
                    renderer.borrow_mut()(self.glow_context.deref());
                    gl.enable(glow::FRAMEBUFFER_SRGB);
                }
            }
        }
        // END MODIFIED

        for (id, image_delta) in textures_delta.set {
            self.painter.set_texture(id, &image_delta);
        }

        let clipped_primitives = egui_ctx.tessellate(shapes);
        let dimensions: [u32; 2] = [canvas_width, canvas_height];

        self.painter
            .paint_primitives(dimensions, pixels_per_point, &clipped_primitives);

        for id in textures_delta.free.drain(..) {
            self.painter.free_texture(id);
        }

        unsafe {
            context.swap_buffers();
            context.make_not_current();
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.painter.destroy()
    }
}