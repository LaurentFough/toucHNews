#![allow(non_snake_case)]
extern crate rubrail;
use rubrail::Touchbar;
use rubrail::TTouchbar;
use rubrail::SpacerType;
use rubrail::SwipeState;

extern crate hn;
use hn::HackerNews;

extern crate open;

#[cfg(feature = "log")]
#[macro_use]
extern crate log;

#[macro_use]
extern crate objc;
use objc::runtime::Object;
use objc::runtime::Class;
use std::cell::Cell;

use std::thread;
use std::time::Duration;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;

struct TouchbarUI {
    touchbar: Touchbar,
    hn: HackerNews,
    headline_label: rubrail::ItemId,
    idx_label: rubrail::ItemId,
    headline_idx: Arc<RwLock<usize>>,
    entries: Vec<hn::Item>,
    rx: Receiver<Cmd>,
}

enum Cmd {
    Open,
    Hide,
}

impl TouchbarUI {
    fn init() -> TouchbarUI {
        let (tx,rx) = channel::<Cmd>();
        let mut touchbar = Touchbar::alloc("hn");
        let hn = HackerNews::new();

        let headline_label = touchbar.create_label("Loading...");
        let headline_idx = Arc::new(RwLock::new(0));
        unsafe { rubrail::util::set_text_color(&headline_label, 1., 1., 1., 1.0); }

        let cb_idx = headline_idx.clone();
        touchbar.add_item_tap_gesture(&headline_label, 1, 1, Box::new(move |_| {
            let mut writer = cb_idx.write().unwrap();
            *writer += 1;
        }));
        touchbar.add_item_swipe_gesture(&headline_label, Box::new(move |item,state,translation| {
            let rgba = match translation {
                t if t > 170. => (0.1, 0.7, 1.0, 1.0),
                t if t < -170. => (0.8, 0.4, 0.1, 1.0),
                _ => (0.9, 0.9, 0.9, 1.0),
            };
            match state {
                SwipeState::Cancelled | SwipeState::Failed | SwipeState::Unknown => {
                    unsafe { rubrail::util::set_text_color(item, 1., 1., 1., 1.); }
                },
                SwipeState::Ended => {
                    unsafe { rubrail::util::set_text_color(item, 1., 1., 1., 1.); }
                    match translation {
                        t if t > 170. => {
                            let _ = tx.send(Cmd::Open);
                        },
                        t if t < -170. => {
                            let _ = tx.send(Cmd::Hide);
                        },
                        _ => {},
                    }
                }
                _ => {
                    unsafe { rubrail::util::set_text_color(item, rgba.0, rgba.1, rgba.2, rgba.3); }
                }
            }
        }));

        let idx_label = touchbar.create_label("0 / 0");
        let cb_idx = headline_idx.clone();
        touchbar.add_item_tap_gesture(&idx_label, 2, 1, Box::new(move |_| {
            let mut writer = cb_idx.write().unwrap();
            *writer = 0;
        }));

        let quit_button = touchbar.create_button(None, Some("X"), Box::new(move |_| {rubrail::app::quit()}));
        touchbar.update_button_width(&quit_button, 30);

        let flexible_space = touchbar.create_spacer(SpacerType::Flexible);
        let root_bar = touchbar.create_bar();

        touchbar.add_items_to_bar(&root_bar, vec![
            headline_label,
            flexible_space,
            idx_label,
            quit_button,
        ]);
        touchbar.set_bar_as_root(root_bar);

        TouchbarUI {
            touchbar: touchbar,
            hn: hn,
            headline_label: headline_label,
            idx_label: idx_label,
            headline_idx: headline_idx,
            entries: vec![],
            rx: rx,
        }
    }
    fn update(&mut self) {
        self.entries = self.hn.into_iter().collect();
        let len = self.entries.len();
        if len == 0 {
            return;
        }
        let idx = *self.headline_idx.read().unwrap();
        if idx >= len {
            let mut writer = self.headline_idx.write().unwrap();
            *writer = 0;
        }
        if let Some(item) = self.entries.get(idx) {
            self.touchbar.update_label(&self.headline_label, &item.title());
            self.touchbar.update_label_width(&self.headline_label, 570);
            self.touchbar.update_label(&self.idx_label, &format!("{}/{}", idx+1, len));
        }
    }
    fn open(&mut self) {
        let idx = *self.headline_idx.read().unwrap();
        if let Some(item) = self.hn.into_iter().nth(idx) {
            let url = item.url();
            let _ = open::that(&url);
        }
    }
    fn hide(&mut self) {
        let idx = *self.headline_idx.read().unwrap();
        if let Some(item) = self.hn.into_iter().nth(idx) {
            self.hn.hide(&item);
        }
        if self.hn.into_iter().count() == 0 {
            self.touchbar.update_label(&self.headline_label, "");
            self.touchbar.update_label_width(&self.headline_label, 570);
            self.touchbar.update_label(&self.idx_label, &format!("{}/{}", 0, 0));
        }
    }
}

fn main() {
    #[cfg(feature = "log")]
    rubrail::app::create_logger(".touchnews.log");
    rubrail::app::init_app();
    let mut bar = TouchbarUI::init();
    let mut nsapp = NSApp::new();
    loop {
        if let Ok(cmd) = bar.rx.recv_timeout(Duration::from_millis(100)) {
            match cmd {
                Cmd::Open => { bar.open(); },
                Cmd::Hide => { bar.hide(); },
            }
        }
        bar.update();
        nsapp.run(false);
    }
}

struct NSApp {
    app: *mut objc::runtime::Object,
    pool: Cell<*mut Object>,
    run_count: Cell<u64>,
    run_mode: *mut Object,
    run_date: *mut Object,
}
impl NSApp {
    fn new() -> NSApp {
        unsafe {
            let cls = Class::get("NSApplication").unwrap();
            let app: *mut Object = msg_send![cls, sharedApplication];
            let cls = Class::get("NSAutoreleasePool").unwrap();
            let pool: *mut Object = msg_send![cls, alloc];
            let pool: *mut Object = msg_send![pool, init];
            let cls = Class::get("NSString").unwrap();
            let rust_runmode = "kCFRunLoopDefaultMode";
            let run_mode: *mut Object = msg_send![cls, alloc];
            let run_mode: *mut Object = msg_send![run_mode,
                                                  initWithBytes:rust_runmode.as_ptr()
                                                  length:rust_runmode.len()
                                                  encoding: 4]; // UTF8_ENCODING
            let date_cls = Class::get("NSDate").unwrap();
            NSApp {
                app: app,
                pool: Cell::new(pool),
                run_count: Cell::new(0),
                run_mode: run_mode,
                run_date: msg_send![date_cls, distantPast],
            }
        }
    }
    fn run(&mut self, block: bool) {
        loop {
            unsafe {
                let run_count = self.run_count.get();
                // Create a new release pool every once in a while, draining the old one
                if run_count % 100 == 0 {
                    let old_pool = self.pool.get();
                    if run_count != 0 {
                        let _ = msg_send![old_pool, drain];
                    }
                    let cls = Class::get("NSAutoreleasePool").unwrap();
                    let pool: *mut Object = msg_send![cls, alloc];
                    let pool: *mut Object = msg_send![pool, init];
                    self.pool.set(pool);
                }
                let mode = self.run_mode;
                let event: *mut Object = msg_send![self.app, nextEventMatchingMask: -1
                                                  untilDate: self.run_date inMode:mode dequeue: 1];
                let _ = msg_send![self.app, sendEvent: event];
                let _ = msg_send![self.app, updateWindows];
                self.run_count.set(run_count + 1);
            }
            if !block { break; }
            thread::sleep(Duration::from_millis(50));
        }
    }
}
