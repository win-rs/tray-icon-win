//! tray-icon-win lets you create tray icons for desktop applications on Windows.
//!
//! This is a personal fork of [tray-icon](https://github.com/tauri-apps/tray-icon). For general use, please consider using the original.
//!
//! # Notes:
//!
//! - An event loop must be running on the thread, on Windows, a win32 event loop. It doesn't need to be the main thread but you have to create the tray icon on the same thread as the event loop.
//!
//! # Examples
//!
//! #### Create a tray icon without a menu.
//!
//! ```no_run
//! use tray_icon_win::{TrayIconBuilder, Icon};
//!
//! # let icon = Icon::from_rgba(Vec::new(), 0, 0).unwrap();
//! let tray_icon = TrayIconBuilder::new()
//!     .with_tooltip("system-tray - tray icon library!")
//!     .with_icon(icon)
//!     .build()
//!     .unwrap();
//! ```
//!
//! #### Create a tray icon with a menu.
//!
//! ```no_run
//! use tray_icon_win::{TrayIconBuilder, menu::Menu,Icon};
//!
//! # let icon = Icon::from_rgba(Vec::new(), 0, 0).unwrap();
//! let tray_menu = Menu::new();
//! let tray_icon = TrayIconBuilder::new()
//!     .with_menu(Box::new(tray_menu))
//!     .with_tooltip("system-tray - tray icon library!")
//!     .with_icon(icon)
//!     .build()
//!     .unwrap();
//! ```
//!
//! # Processing tray events
//!
//! You can use [`TrayIconEvent::receiver`] to get a reference to the [`TrayIconEventReceiver`]
//! which you can use to listen to events when a click happens on the tray icon
//! ```no_run
//! use tray_icon_win::TrayIconEvent;
//!
//! if let Ok(event) = TrayIconEvent::receiver().try_recv() {
//!     println!("{:?}", event);
//! }
//! ```
//!
//! You can also listen for the menu events using [`MenuEvent::receiver`](crate::menu::MenuEvent::receiver) to get events for the tray context menu.
//!
//! ```no_run
//! use tray_icon_win::{TrayIconEvent, menu::MenuEvent};
//!
//! if let Ok(event) = TrayIconEvent::receiver().try_recv() {
//!     println!("tray event: {:?}", event);
//! }
//!
//! if let Ok(event) = MenuEvent::receiver().try_recv() {
//!     println!("menu event: {:?}", event);
//! }
//! ```
//!
//! ### Note for [winit] or [tao] users:
//!
//! You should use [`TrayIconEvent::set_event_handler`] and forward
//! the tray icon events to the event loop by using [`EventLoopProxy`]
//! so that the event loop is awakened on each tray icon event.
//! Same can be done for menu events using [`MenuEvent::set_event_handler`].
//!
//! ```no_run
//! # use winit::event_loop::EventLoop;
//! enum UserEvent {
//!   TrayIconEvent(tray_icon_win::TrayIconEvent),
//!   MenuEvent(tray_icon_win::menu::MenuEvent)
//! }
//!
//! let event_loop = EventLoop::<UserEvent>::with_user_event().build().unwrap();
//!
//! let proxy = event_loop.create_proxy();
//! tray_icon_win::TrayIconEvent::set_event_handler(Some(move |event| {
//!     proxy.send_event(UserEvent::TrayIconEvent(event));
//! }));
//!
//! let proxy = event_loop.create_proxy();
//! tray_icon_win::menu::MenuEvent::set_event_handler(Some(move |event| {
//!     proxy.send_event(UserEvent::MenuEvent(event));
//! }));
//! ```
//!
//! [`EventLoopProxy`]: https://docs.rs/winit/latest/winit/event_loop/struct.EventLoopProxy.html
//! [winit]: https://docs.rs/winit
//! [tao]: https://docs.rs/tao

use std::{cell::RefCell, rc::Rc};

use counter::Counter;
use crossbeam_channel::{unbounded, Receiver, Sender};
use platform_impl::TrayIcon as PlatformTrayIcon;
use std::sync::{LazyLock, OnceLock};

mod counter;
mod error;
mod icon;
mod platform_impl;
mod tray_icon_id;

pub use self::error::*;
pub use self::icon::{BadIcon, Icon};
pub use self::tray_icon_id::TrayIconId;

/// Re-export of [muda](::muda) crate and used for tray context menu.
pub mod menu {
    pub use muda::*;
}
pub use muda::dpi;

static COUNTER: Counter = Counter::new();

/// Attributes to use when creating a tray icon.
pub struct TrayIconAttributes {
    /// Tray icon tooltip
    pub tooltip: Option<String>,

    /// Tray menu
    pub menu: Option<Box<dyn menu::ContextMenu>>,

    /// Tray icon
    pub icon: Option<Icon>,

    /// Whether to show the tray menu on left click or not, default is `true`.
    pub menu_on_left_click: bool,
}

impl Default for TrayIconAttributes {
    fn default() -> Self {
        Self {
            tooltip: None,
            menu: None,
            icon: None,
            menu_on_left_click: true,
        }
    }
}

/// [`TrayIcon`] builder struct and associated methods.
#[derive(Default)]
pub struct TrayIconBuilder {
    id: TrayIconId,
    attrs: TrayIconAttributes,
}

impl TrayIconBuilder {
    /// Creates a new [`TrayIconBuilder`] with default [`TrayIconAttributes`].
    ///
    /// See [`TrayIcon::new`] for more info.
    pub fn new() -> Self {
        Self {
            id: TrayIconId(COUNTER.next().to_string()),
            attrs: TrayIconAttributes::default(),
        }
    }

    /// Sets the unique id to build the tray icon with.
    pub fn with_id<I: Into<TrayIconId>>(mut self, id: I) -> Self {
        self.id = id.into();
        self
    }

    /// Set the a menu for this tray icon.
    pub fn with_menu(mut self, menu: Box<dyn menu::ContextMenu>) -> Self {
        self.attrs.menu = Some(menu);
        self
    }

    /// Set an icon for this tray icon.
    pub fn with_icon(mut self, icon: Icon) -> Self {
        self.attrs.icon = Some(icon);
        self
    }

    /// Set a tooltip for this tray icon.
    pub fn with_tooltip<S: AsRef<str>>(mut self, s: S) -> Self {
        self.attrs.tooltip = Some(s.as_ref().to_string());
        self
    }

    /// Whether to show the tray menu on left click or not, default is `true`. **macOS only**.
    pub fn with_menu_on_left_click(mut self, enable: bool) -> Self {
        self.attrs.menu_on_left_click = enable;
        self
    }

    /// Access the unique id that will be assigned to the tray icon
    /// this builder will create.
    pub fn id(&self) -> &TrayIconId {
        &self.id
    }

    /// Builds and adds a new [`TrayIcon`] to the system tray.
    pub fn build(self) -> Result<TrayIcon> {
        TrayIcon::with_id(self.id, self.attrs)
    }
}

// Tray icon struct and associated methods.
///
/// This type is reference-counted and the icon is removed when the last instance is dropped.
#[derive(Clone)]
pub struct TrayIcon {
    id: TrayIconId,
    tray: Rc<RefCell<PlatformTrayIcon>>,
}

impl TrayIcon {
    /// Builds and adds a new tray icon to the system tray.
    pub fn new(attrs: TrayIconAttributes) -> Result<Self> {
        let id = TrayIconId(COUNTER.next().to_string());
        Ok(Self {
            tray: Rc::new(RefCell::new(PlatformTrayIcon::new(id.clone(), attrs)?)),
            id,
        })
    }

    /// Builds and adds a new tray icon to the system tray with the specified Id.
    ///
    /// See [`TrayIcon::new`] for more info.
    pub fn with_id<I: Into<TrayIconId>>(id: I, attrs: TrayIconAttributes) -> Result<Self> {
        let id = id.into();
        Ok(Self {
            tray: Rc::new(RefCell::new(PlatformTrayIcon::new(id.clone(), attrs)?)),
            id,
        })
    }

    /// Returns the id associated with this tray icon.
    pub fn id(&self) -> &TrayIconId {
        &self.id
    }

    /// Set new tray icon. If `None` is provided, it will remove the icon.
    pub fn set_icon(&self, icon: Option<Icon>) -> Result<()> {
        self.tray.borrow_mut().set_icon(icon)
    }

    /// Set new tray menu.
    pub fn set_menu(&self, menu: Option<Box<dyn menu::ContextMenu>>) {
        self.tray.borrow_mut().set_menu(menu)
    }

    /// Sets the tooltip for this tray icon.
    pub fn set_tooltip<S: AsRef<str>>(&self, tooltip: Option<S>) -> Result<()> {
        self.tray.borrow_mut().set_tooltip(tooltip)
    }

    /// Show or hide this tray icon
    pub fn set_visible(&self, visible: bool) -> Result<()> {
        self.tray.borrow_mut().set_visible(visible)
    }

    /// Disable or enable showing the tray menu on left click.
    pub fn set_show_menu_on_left_click(&self, enable: bool) {
        self.tray.borrow_mut().set_show_menu_on_left_click(enable);
    }

    /// Get tray icon rect.
    pub fn rect(&self) -> Option<Rect> {
        self.tray.borrow().rect()
    }
}

/// Describes a tray icon event.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
#[non_exhaustive]
pub enum TrayIconEvent {
    /// A click happened on the tray icon.
    #[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
    Click {
        /// Id of the tray icon which triggered this event.
        id: TrayIconId,
        /// Physical Position of this event.
        position: dpi::PhysicalPosition<f64>,
        /// Position and size of the tray icon.
        rect: Rect,
        /// Mouse button that triggered this event.
        button: MouseButton,
        /// Mouse button state when this event was triggered.
        button_state: MouseButtonState,
    },
    /// A double click happened on the tray icon. **Windows Only**
    DoubleClick {
        /// Id of the tray icon which triggered this event.
        id: TrayIconId,
        /// Physical Position of this event.
        position: dpi::PhysicalPosition<f64>,
        /// Position and size of the tray icon.
        rect: Rect,
        /// Mouse button that triggered this event.
        button: MouseButton,
    },
    /// The mouse entered the tray icon region.
    Enter {
        /// Id of the tray icon which triggered this event.
        id: TrayIconId,
        /// Physical Position of this event.
        position: dpi::PhysicalPosition<f64>,
        /// Position and size of the tray icon.
        rect: Rect,
    },
    /// The mouse moved over the tray icon region.
    Move {
        /// Id of the tray icon which triggered this event.
        id: TrayIconId,
        /// Physical Position of this event.
        position: dpi::PhysicalPosition<f64>,
        /// Position and size of the tray icon.
        rect: Rect,
    },
    /// The mouse left the tray icon region.
    Leave {
        /// Id of the tray icon which triggered this event.
        id: TrayIconId,
        /// Physical Position of this event.
        position: dpi::PhysicalPosition<f64>,
        /// Position and size of the tray icon.
        rect: Rect,
    },
}

/// Describes the mouse button state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MouseButtonState {
    Up,
    Down,
}

impl Default for MouseButtonState {
    fn default() -> Self {
        Self::Up
    }
}

/// Describes which mouse button triggered the event..
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl Default for MouseButton {
    fn default() -> Self {
        Self::Left
    }
}

/// Describes a rectangle including position (x - y axis) and size.
#[derive(Debug, PartialEq, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rect {
    pub size: dpi::PhysicalSize<u32>,
    pub position: dpi::PhysicalPosition<f64>,
}

impl Default for Rect {
    fn default() -> Self {
        Self {
            size: dpi::PhysicalSize::new(0, 0),
            position: dpi::PhysicalPosition::new(0., 0.),
        }
    }
}

/// A reciever that could be used to listen to tray events.
pub type TrayIconEventReceiver = Receiver<TrayIconEvent>;
type TrayIconEventHandler = Box<dyn Fn(TrayIconEvent) + Send + Sync + 'static>;

static TRAY_CHANNEL: LazyLock<(Sender<TrayIconEvent>, TrayIconEventReceiver)> =
    LazyLock::new(unbounded);
static TRAY_EVENT_HANDLER: OnceLock<Option<TrayIconEventHandler>> = OnceLock::new();

impl TrayIconEvent {
    /// Returns the id of the tray icon which triggered this event.
    pub fn id(&self) -> &TrayIconId {
        match self {
            TrayIconEvent::Click { id, .. } => id,
            TrayIconEvent::DoubleClick { id, .. } => id,
            TrayIconEvent::Enter { id, .. } => id,
            TrayIconEvent::Move { id, .. } => id,
            TrayIconEvent::Leave { id, .. } => id,
        }
    }

    /// Gets a reference to the event channel's [`TrayIconEventReceiver`]
    /// which can be used to listen for tray events.
    ///
    /// ## Note
    ///
    /// This will not receive any events if [`TrayIconEvent::set_event_handler`] has been called with a `Some` value.
    pub fn receiver<'a>() -> &'a TrayIconEventReceiver {
        &TRAY_CHANNEL.1
    }

    /// Set a handler to be called for new events. Useful for implementing custom event sender.
    ///
    /// ## Note
    ///
    /// Calling this function with a `Some` value,
    /// will not send new events to the channel associated with [`TrayIconEvent::receiver`]
    pub fn set_event_handler<F: Fn(TrayIconEvent) + Send + Sync + 'static>(f: Option<F>) {
        if let Some(f) = f {
            let _ = TRAY_EVENT_HANDLER.set(Some(Box::new(f)));
        } else {
            let _ = TRAY_EVENT_HANDLER.set(None);
        }
    }

    #[allow(unused)]
    pub(crate) fn send(event: TrayIconEvent) {
        if let Some(handler) = TRAY_EVENT_HANDLER.get_or_init(|| None) {
            handler(event);
        } else {
            let _ = TRAY_CHANNEL.0.send(event);
        }
    }
}

#[cfg(test)]
mod tests {

    #[cfg(feature = "serde")]
    #[test]
    fn it_serializes() {
        use super::*;
        let event = TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Down,
            id: TrayIconId::new("id"),
            position: dpi::PhysicalPosition::default(),
            rect: Rect::default(),
        };

        let value = serde_jsonc2::to_value(&event).unwrap();
        assert_eq!(
            value,
            serde_jsonc2::jsonc!({
                "type": "Click",
                "button": "Left",
                "buttonState": "Down",
                "id": "id",
                "position": {
                    "x": 0.0,
                    "y": 0.0,
                },
                "rect": {
                    "size": {
                        "width": 0,
                        "height": 0,
                    },
                    "position": {
                        "x": 0.0,
                        "y": 0.0,
                    },
                }
            })
        )
    }
}
