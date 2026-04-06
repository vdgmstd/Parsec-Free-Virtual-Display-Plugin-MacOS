//! Native macOS system tray (status bar) using objc2
//!
//! Uses typed function pointers for ARM64/x64 compatibility
//! Sends commands via the central command system (DRY architecture)

#![allow(dead_code)]
#![allow(unused_imports)]

use objc2::rc::Retained;
use objc2::{AllocAnyThread, ClassType};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSImage, NSMenu, NSMenuItem, NSStatusBar,
    NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{MainThreadMarker, NSData, NSSize, NSString};
use std::ffi::{c_void, CString};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tracing::info;

// Embed tray icons at compile time
const TRAY_ICON_INACTIVE: &[u8] = include_bytes!("../assets/tray-inactive.svg");
const TRAY_ICON_ACTIVE: &[u8] = include_bytes!("../assets/tray-active.svg");

use crate::commands::{push_command, Command};
use crate::settings::{Settings, FRAME_RATES, RESOLUTIONS};

// Current state (for menu checkmarks)
pub static CURRENT_WIDTH: AtomicU32 = AtomicU32::new(3440);
pub static CURRENT_HEIGHT: AtomicU32 = AtomicU32::new(1440);
pub static CURRENT_FPS: AtomicU32 = AtomicU32::new(60);
pub static IS_CONNECTED: AtomicBool = AtomicBool::new(false);

// Typed function pointers for objc_msgSend (ARM64 compatible)
mod objc_typed {
    use std::ffi::{c_void, CString};
    use std::ptr;
    use std::sync::Once;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const i8) -> *mut c_void;
        fn objc_allocateClassPair(
            superclass: *mut c_void,
            name: *const i8,
            extra_bytes: usize,
        ) -> *mut c_void;
        fn objc_registerClassPair(cls: *mut c_void);
        fn class_addMethod(
            cls: *mut c_void,
            name: *mut c_void,
            imp: *mut c_void,
            types: *const i8,
        ) -> bool;
        fn sel_registerName(name: *const i8) -> *mut c_void;
        pub fn objc_msgSend();
    }

    // Typed function signatures
    type MsgSend0 = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
    type MsgSend1Ptr = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void;
    type MsgSendIsize = unsafe extern "C" fn(*mut c_void, *mut c_void) -> isize;

    pub fn get_class(name: &str) -> *mut c_void {
        let name_cstr = CString::new(name).unwrap();
        unsafe { objc_getClass(name_cstr.as_ptr()) }
    }

    pub fn get_selector(name: &str) -> *mut c_void {
        let name_cstr = CString::new(name).unwrap();
        unsafe { sel_registerName(name_cstr.as_ptr()) }
    }

    #[inline]
    pub unsafe fn msg_send0(obj: *mut c_void, sel: *mut c_void) -> *mut c_void {
        unsafe {
            let f: MsgSend0 = std::mem::transmute(objc_msgSend as *const ());
            f(obj, sel)
        }
    }

    #[inline]
    pub unsafe fn msg_send1_ptr(
        obj: *mut c_void,
        sel: *mut c_void,
        arg: *mut c_void,
    ) -> *mut c_void {
        unsafe {
            let f: MsgSend1Ptr = std::mem::transmute(objc_msgSend as *const ());
            f(obj, sel, arg)
        }
    }

    #[inline]
    pub unsafe fn msg_send_get_isize(obj: *mut c_void, sel: *mut c_void) -> isize {
        unsafe {
            let f: MsgSendIsize = std::mem::transmute(objc_msgSend as *const ());
            f(obj, sel)
        }
    }

    // Handler class
    static HANDLER_INIT: Once = Once::new();
    static mut HANDLER_CLASS: *mut c_void = ptr::null_mut();
    static mut HANDLER_INSTANCE: *mut c_void = ptr::null_mut();

    extern "C" fn resolution_action(_self: *mut c_void, _cmd: *mut c_void, sender: *mut c_void) {
        unsafe {
            let tag_sel = get_selector("tag");
            let tag = msg_send_get_isize(sender, tag_sel);

            let width = (tag / 10000) as u32;
            let height = (tag % 10000) as u32;

            tracing::info!("[Tray] Resolution selected: {}x{}", width, height);

            // Update state
            super::CURRENT_WIDTH.store(width, std::sync::atomic::Ordering::SeqCst);
            super::CURRENT_HEIGHT.store(height, std::sync::atomic::Ordering::SeqCst);

            // Send command (DRY - no logic duplication)
            crate::commands::push_command(crate::commands::Command::SetResolution(width, height));
        }
    }

    extern "C" fn fps_action(_self: *mut c_void, _cmd: *mut c_void, sender: *mut c_void) {
        unsafe {
            let tag_sel = get_selector("tag");
            let tag = msg_send_get_isize(sender, tag_sel);
            let fps = tag as u32;

            tracing::info!("[Tray] FPS selected: {}", fps);

            // Update state
            super::CURRENT_FPS.store(fps, std::sync::atomic::Ordering::SeqCst);

            // Send command (DRY - no logic duplication)
            crate::commands::push_command(crate::commands::Command::SetFps(fps));
        }
    }

    extern "C" fn show_settings_action(_self: *mut c_void, _cmd: *mut c_void, _sender: *mut c_void) {
        tracing::info!("[Tray] Show Settings clicked");

        unsafe {
            let nsapp = get_class("NSApplication");
            let shared_app_sel = get_selector("sharedApplication");
            let app = msg_send0(nsapp, shared_app_sel);

            // activateIgnoringOtherApps:YES
            let activate_sel = get_selector("activateIgnoringOtherApps:");
            type MsgSendBool = unsafe extern "C" fn(*mut c_void, *mut c_void, i8);
            let f: MsgSendBool = std::mem::transmute(objc_msgSend as *const ());
            f(app, activate_sel, 1); // YES = 1

            // Get windows array
            let windows_sel = get_selector("windows");
            let windows = msg_send0(app, windows_sel);

            // Get first window (our main window)
            let first_obj_sel = get_selector("firstObject");
            let window = msg_send0(windows, first_obj_sel);

            if !window.is_null() {
                // makeKeyAndOrderFront:nil - shows and focuses window
                let make_key_sel = get_selector("makeKeyAndOrderFront:");
                msg_send1_ptr(window, make_key_sel, std::ptr::null_mut());

                tracing::info!("[Tray] Window shown via makeKeyAndOrderFront");
            } else {
                tracing::warn!("[Tray] No window found");
            }
        }
    }

    pub fn init_handler_class() {
        HANDLER_INIT.call_once(|| unsafe {
            let nsobject = get_class("NSObject");
            let class_name = CString::new("TrayActionHandler").unwrap();
            HANDLER_CLASS = objc_allocateClassPair(nsobject, class_name.as_ptr(), 0);

            if !HANDLER_CLASS.is_null() {
                let res_sel = get_selector("resolutionAction:");
                let fps_sel = get_selector("fpsAction:");
                let show_sel = get_selector("showSettingsAction:");
                let types = CString::new("v@:@").unwrap();

                class_addMethod(
                    HANDLER_CLASS,
                    res_sel,
                    resolution_action as *mut c_void,
                    types.as_ptr(),
                );
                class_addMethod(
                    HANDLER_CLASS,
                    fps_sel,
                    fps_action as *mut c_void,
                    types.as_ptr(),
                );
                class_addMethod(
                    HANDLER_CLASS,
                    show_sel,
                    show_settings_action as *mut c_void,
                    types.as_ptr(),
                );

                objc_registerClassPair(HANDLER_CLASS);

                let alloc_sel = get_selector("alloc");
                let init_sel = get_selector("init");
                let obj = msg_send0(HANDLER_CLASS, alloc_sel);
                HANDLER_INSTANCE = msg_send0(obj, init_sel);
            }
        });
    }

    pub fn get_handler_instance() -> *mut c_void {
        init_handler_class();
        unsafe { HANDLER_INSTANCE }
    }

    pub fn get_resolution_selector() -> *mut c_void {
        get_selector("resolutionAction:")
    }

    pub fn get_fps_selector() -> *mut c_void {
        get_selector("fpsAction:")
    }

    pub fn get_show_settings_selector() -> *mut c_void {
        get_selector("showSettingsAction:")
    }
}

pub struct NativeTray {
    status_item: Retained<NSStatusItem>,
    mtm: MainThreadMarker,
}

impl NativeTray {
    pub fn new(mtm: MainThreadMarker) -> Self {
        objc_typed::init_handler_class();

        if let Ok(settings) = Settings::load() {
            CURRENT_WIDTH.store(settings.width, Ordering::SeqCst);
            CURRENT_HEIGHT.store(settings.height, Ordering::SeqCst);
            CURRENT_FPS.store(settings.frame_rate, Ordering::SeqCst);
        }

        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

        let status_bar = NSStatusBar::systemStatusBar();
        let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);

        if let Some(button) = status_item.button(mtm) {
            if let Some(image) = Self::load_icon(false) {
                button.setImage(Some(&image));
            }
        }

        let menu = Self::create_menu(mtm);
        menu.setAutoenablesItems(false);
        status_item.setMenu(Some(&menu));

        info!("Native tray icon created with full menu");

        Self { status_item, mtm }
    }

    fn load_icon(connected: bool) -> Option<Retained<NSImage>> {
        // Load embedded SVG icons
        let icon_data = if connected {
            TRAY_ICON_ACTIVE
        } else {
            TRAY_ICON_INACTIVE
        };

        // Create NSData from embedded bytes
        let ns_data = unsafe { NSData::dataWithBytes_length(icon_data.as_ptr() as *const c_void, icon_data.len()) };

        // Create NSImage from data
        if let Some(image) = NSImage::initWithData(NSImage::alloc(), &ns_data) {
            image.setTemplate(true);  // Adapts to light/dark menu bar
            image.setSize(NSSize {
                width: 18.0,
                height: 18.0,
            });
            return Some(image);
        }

        // Fallback to SF Symbol if SVG fails
        let symbol_name = NSString::from_str("display");
        let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(&symbol_name, None);
        if let Some(ref img) = image {
            img.setTemplate(true);
            img.setSize(NSSize {
                width: 18.0,
                height: 18.0,
            });
        }
        image
    }

    /// Set action and target on menu item using typed objc calls
    unsafe fn set_menu_item_action(item: &NSMenuItem, action: *mut c_void, target: *mut c_void) {
        unsafe {
            let set_action_sel = objc_typed::get_selector("setAction:");
            let set_target_sel = objc_typed::get_selector("setTarget:");

            let item_ptr = item as *const NSMenuItem as *mut c_void;
            objc_typed::msg_send1_ptr(item_ptr, set_action_sel, action);
            objc_typed::msg_send1_ptr(item_ptr, set_target_sel, target);
        }
    }

    fn create_menu(mtm: MainThreadMarker) -> Retained<NSMenu> {
        let menu = NSMenu::new(mtm);

        let handler = objc_typed::get_handler_instance();
        let res_selector = objc_typed::get_resolution_selector();
        let fps_selector = objc_typed::get_fps_selector();

        // Status item
        let status_title = NSString::from_str("Status: Monitoring...");
        let status_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc::<NSMenuItem>(),
                &status_title,
                None,
                &NSString::from_str(""),
            )
        };
        status_item.setEnabled(false);
        menu.addItem(&status_item);

        menu.addItem(&NSMenuItem::separatorItem(mtm));

        // Resolution submenu
        let res_title = NSString::from_str("Resolution");
        let res_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc::<NSMenuItem>(),
                &res_title,
                None,
                &NSString::from_str(""),
            )
        };

        let res_submenu = NSMenu::new(mtm);
        res_submenu.setAutoenablesItems(false);

        let current_w = CURRENT_WIDTH.load(Ordering::SeqCst);
        let current_h = CURRENT_HEIGHT.load(Ordering::SeqCst);

        for &(w, h, name) in RESOLUTIONS.iter() {
            let item_title = NSString::from_str(name);
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    mtm.alloc::<NSMenuItem>(),
                    &item_title,
                    None,
                    &NSString::from_str(""),
                )
            };
            item.setEnabled(true);
            item.setTag((w as isize) * 10000 + (h as isize));

            unsafe {
                Self::set_menu_item_action(&item, res_selector, handler);
            }

            if w == current_w && h == current_h {
                item.setState(1);
            }

            res_submenu.addItem(&item);
        }

        // Custom resolutions
        if let Ok(settings) = Settings::load() {
            if !settings.custom_resolutions.is_empty() {
                res_submenu.addItem(&NSMenuItem::separatorItem(mtm));

                for custom in settings.custom_resolutions.iter() {
                    let item_title = NSString::from_str(&custom.name);
                    let item = unsafe {
                        NSMenuItem::initWithTitle_action_keyEquivalent(
                            mtm.alloc::<NSMenuItem>(),
                            &item_title,
                            None,
                            &NSString::from_str(""),
                        )
                    };
                    item.setEnabled(true);
                    item.setTag((custom.width as isize) * 10000 + (custom.height as isize));

                    unsafe {
                        Self::set_menu_item_action(&item, res_selector, handler);
                    }

                    if custom.width == current_w && custom.height == current_h {
                        item.setState(1);
                    }

                    res_submenu.addItem(&item);
                }
            }
        }

        res_item.setSubmenu(Some(&res_submenu));
        menu.addItem(&res_item);

        // FPS submenu
        let fps_title = NSString::from_str("Frame Rate");
        let fps_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc::<NSMenuItem>(),
                &fps_title,
                None,
                &NSString::from_str(""),
            )
        };

        let fps_submenu = NSMenu::new(mtm);
        fps_submenu.setAutoenablesItems(false);

        let current_fps = CURRENT_FPS.load(Ordering::SeqCst);

        for &fps in FRAME_RATES.iter() {
            let item_title = NSString::from_str(&format!("{} FPS", fps));
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    mtm.alloc::<NSMenuItem>(),
                    &item_title,
                    None,
                    &NSString::from_str(""),
                )
            };
            item.setEnabled(true);
            item.setTag(fps as isize);

            unsafe {
                Self::set_menu_item_action(&item, fps_selector, handler);
            }

            if fps == current_fps {
                item.setState(1);
            }

            fps_submenu.addItem(&item);
        }

        fps_item.setSubmenu(Some(&fps_submenu));
        menu.addItem(&fps_item);

        menu.addItem(&NSMenuItem::separatorItem(mtm));

        // Show Settings
        let settings_title = NSString::from_str("Show Settings...");
        let settings_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc::<NSMenuItem>(),
                &settings_title,
                None,
                &NSString::from_str(","),
            )
        };
        settings_item.setEnabled(true);
        let show_selector = objc_typed::get_show_settings_selector();
        unsafe {
            Self::set_menu_item_action(&settings_item, show_selector, handler);
        }
        menu.addItem(&settings_item);

        menu.addItem(&NSMenuItem::separatorItem(mtm));

        // Quit
        let quit_title = NSString::from_str("Quit");
        let quit_item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc::<NSMenuItem>(),
                &quit_title,
                Some(objc2::sel!(terminate:)),
                &NSString::from_str("q"),
            )
        };
        quit_item.setEnabled(true);
        menu.addItem(&quit_item);

        menu
    }

    pub fn set_status(&self, connected: bool, username: Option<&str>) {
        IS_CONNECTED.store(connected, Ordering::SeqCst);

        if let Some(menu) = self.status_item.menu(self.mtm) {
            if let Some(item) = menu.itemAtIndex(0) {
                let status_text = if connected {
                    format!("Connected: {}", username.unwrap_or("Unknown"))
                } else {
                    "Status: Monitoring...".to_string()
                };
                item.setTitle(&NSString::from_str(&status_text));
            }
        }
    }

    pub fn set_connected(&self, connected: bool) {
        IS_CONNECTED.store(connected, Ordering::SeqCst);

        if let Some(button) = self.status_item.button(self.mtm) {
            if let Some(image) = Self::load_icon(connected) {
                button.setImage(Some(&image));
            }
        }
    }

    pub fn set_resolution(&self, width: u32, height: u32) {
        CURRENT_WIDTH.store(width, Ordering::SeqCst);
        CURRENT_HEIGHT.store(height, Ordering::SeqCst);

        if let Some(menu) = self.status_item.menu(self.mtm) {
            if let Some(res_item) = menu.itemAtIndex(2) {
                if let Some(submenu) = res_item.submenu() {
                    let count = submenu.numberOfItems();
                    for i in 0..count {
                        if let Some(item) = submenu.itemAtIndex(i) {
                            let tag = item.tag();
                            if tag > 0 {
                                let item_w = (tag / 10000) as u32;
                                let item_h = (tag % 10000) as u32;
                                item.setState(if item_w == width && item_h == height {
                                    1
                                } else {
                                    0
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn set_fps(&self, fps: u32) {
        CURRENT_FPS.store(fps, Ordering::SeqCst);

        if let Some(menu) = self.status_item.menu(self.mtm) {
            if let Some(fps_item) = menu.itemAtIndex(3) {
                if let Some(submenu) = fps_item.submenu() {
                    for (i, &f) in FRAME_RATES.iter().enumerate() {
                        if let Some(item) = submenu.itemAtIndex(i as isize) {
                            item.setState(if f == fps { 1 } else { 0 });
                        }
                    }
                }
            }
        }
    }
}
