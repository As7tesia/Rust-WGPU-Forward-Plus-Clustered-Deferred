// Each `mod` line pulls in a source file: `mod app;` is src/app.rs, `mod stage;` is
// src/stage/mod.rs, and so on.
mod app;
mod math_util;
mod renderer;
mod renderers;
mod shaders;
mod stage;

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use app::App;

/// Receives winit's events and passes them to the `App`.
///
/// `App` needs a window to create its GPU surface, and winit only creates windows once the
/// event loop runs (in `resumed`), so the handler starts out without one.
struct Handler {
    app: Option<App>,
    /// Lets the async setup on the web hand the finished `App` back to the event loop.
    #[cfg(target_arch = "wasm32")]
    proxy: Option<winit::event_loop::EventLoopProxy<App>>,
}

// `ApplicationHandler<App>`: the type parameter is the "user event" type, which the web build
// uses to deliver the `App` once its async setup is done.
impl ApplicationHandler<App> for Handler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.app.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(window_attributes())
                .expect("failed to create the window"),
        );

        // `#[cfg(...)]` compiles the next item only for the matching target, like `#ifdef`.

        // Desktop: `pollster::block_on` runs the async setup to completion on this thread.
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.app = Some(pollster::block_on(App::new(window)));
        }

        // Web: the browser's main thread can't block, so the setup runs as a background task
        // and sends the `App` back through the proxy, arriving in `user_event` below.
        #[cfg(target_arch = "wasm32")]
        if let Some(proxy) = self.proxy.take() {
            wasm_bindgen_futures::spawn_local(async move {
                let app = App::new(window).await;
                // This only fails if the event loop has already exited.
                let _ = proxy.send_event(app);
            });
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, mut app: App) {
        app.resize_to_window();
        self.app = Some(app);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(app) = &mut self.app {
            app.on_window_event(event_loop, event);
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let Some(app) = &mut self.app {
            app.on_device_event(event);
        }
    }
}

fn window_attributes() -> WindowAttributes {
    let attributes = Window::default_attributes().with_title("cool and awesome renderer");

    #[cfg(not(target_arch = "wasm32"))]
    let attributes = attributes.with_inner_size(winit::dpi::LogicalSize::new(1600.0, 900.0));

    // On the web, draw into the <canvas id="mainCanvas"> from index.html. Its size comes from
    // the page's CSS (the whole browser window).
    #[cfg(target_arch = "wasm32")]
    let attributes = {
        use wasm_bindgen::JsCast;
        use winit::platform::web::WindowAttributesExtWebSys;

        let canvas = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id("mainCanvas"))
            .expect("no element with id mainCanvas in index.html")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("mainCanvas is not a <canvas>");
        attributes.with_canvas(Some(canvas))
    };

    attributes
}

fn main() {
    // Log at info level for this crate and warn for everything else. Set the RUST_LOG
    // environment variable to change that, e.g. `RUST_LOG=wgpu_core=info`.
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn,forward_plus_clustered_deferred=info"),
    )
    .init();

    #[cfg(target_arch = "wasm32")]
    {
        // Print Rust panic messages to the browser console instead of a bare
        // "unreachable executed", and send `log` output there too.
        console_error_panic_hook::set_once();
        console_log::init_with_level(log::Level::Info).expect("failed to set up logging");
    }

    let event_loop = EventLoop::<App>::with_user_event()
        .build()
        .expect("failed to create the event loop");

    // Desktop: `run_app` blocks until the window closes.
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut handler = Handler { app: None };
        event_loop.run_app(&mut handler).expect("event loop error");
    }

    // Web: `spawn_app` returns right away and the browser drives the event loop from then on.
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;

        let handler = Handler {
            app: None,
            proxy: Some(event_loop.create_proxy()),
        };
        event_loop.spawn_app(handler);
    }
}
