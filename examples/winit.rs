#![allow(unused)]

use tray_icon_win::{
    menu::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder, TrayIconEvent,
};
use winit::{
    event::{Event, StartCause},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

use winit::application::ApplicationHandler;

const WAIT_TIME: std::time::Duration = std::time::Duration::from_millis(100);
const POLL_SLEEP_TIME: std::time::Duration = std::time::Duration::from_millis(100);

#[derive(Debug)]
enum UserEvent {
    TrayIconEvent(tray_icon_win::TrayIconEvent),
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    #[default]
    Wait,
    WaitUntil,
    Poll,
}

#[derive(Default)]
struct ControlFlowDemo {
    mode: Mode,
    request_redraw: bool,
    wait_cancelled: bool,
    close_requested: bool,
    tray_icon: Option<TrayIcon>,
    window: Option<Window>,
    path: String,
}

impl ControlFlowDemo {
    pub fn new(path: &str, tray_icon: Option<TrayIcon>) -> Self {
        Self {
            path: path.to_string(),
            tray_icon,
            ..Default::default()
        }
    }
}

impl ApplicationHandler<UserEvent> for ControlFlowDemo {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if cause == StartCause::Init {
            let icon = load_icon(std::path::Path::new(&self.path));

            // We create the icon once the event loop is actually running
            // to prevent issues like https://github.com/tauri-apps/tray-icon/issues/90
            self.tray_icon = Some(
                TrayIconBuilder::new()
                    .with_menu(Box::new(Menu::new()))
                    .with_tooltip("winit - awesome windowing lib")
                    .with_icon(icon)
                    .build()
                    .unwrap(),
            );
        }
    }
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes().with_title(
            "Press 1, 2, 3 to change control flow mode. Press R to toggle redraw requests.",
        );
        self.window = Some(event_loop.create_window(window_attributes).unwrap());
    }
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: winit::event::WindowEvent,
    ) {
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.request_redraw && !self.wait_cancelled && !self.close_requested {
            self.window.as_ref().unwrap().request_redraw();
        }

        match self.mode {
            Mode::Wait => event_loop.set_control_flow(ControlFlow::Wait),
            Mode::WaitUntil => {
                if !self.wait_cancelled {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(
                        std::time::Instant::now() + WAIT_TIME,
                    ));
                }
            }
            Mode::Poll => {
                std::thread::sleep(POLL_SLEEP_TIME);
                event_loop.set_control_flow(ControlFlow::Poll);
            }
        };

        if self.close_requested {
            event_loop.exit();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        println!("{event:?}");
    }
}

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/icon.png");

    let event_loop = EventLoop::<UserEvent>::with_user_event().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);

    // set a tray event handler that forwards the event and wakes up the event loop
    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        proxy.send_event(UserEvent::TrayIconEvent(event));
    }));

    #[cfg(not(target_os = "linux"))]
    let mut tray_icon = None;

    let menu_channel = MenuEvent::receiver();
    let tray_channel = TrayIconEvent::receiver();

    let mut app = ControlFlowDemo::new(path, tray_icon);
    event_loop.run_app(&mut app);
}

fn load_icon(path: &std::path::Path) -> tray_icon_win::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon_win::Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}
