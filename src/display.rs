//! Virtual Display management using pure Rust
//!
//! Uses typed function pointers for objc_msgSend to support both x64 and ARM64

use anyhow::{anyhow, Result};
use tracing::{info, warn};

/// A single resolution mode
#[derive(Debug, Clone)]
pub struct ResolutionMode {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct DisplayConfig {
    pub width: u32,
    pub height: u32,
    pub frame_rate: u32,
    pub name: String,
    /// All available resolutions (will be added as display modes)
    pub available_modes: Vec<ResolutionMode>,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            width: 3440,
            height: 1440,
            frame_rate: 60,
            name: "Parsec".to_string(),
            available_modes: vec![
                ResolutionMode { width: 1280, height: 720 },
                ResolutionMode { width: 1920, height: 1080 },
                ResolutionMode { width: 2560, height: 1440 },
                ResolutionMode { width: 3440, height: 1440 },
                ResolutionMode { width: 3840, height: 2160 },
            ],
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::cell::RefCell;
    use std::ffi::{c_void, CString};
    use std::ptr;

    pub type CGDirectDisplayID = u32;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        pub fn CGMainDisplayID() -> CGDirectDisplayID;
        pub fn CGDisplayMirrorsDisplay(display: CGDirectDisplayID) -> CGDirectDisplayID;
        pub fn CGDisplayIsInMirrorSet(display: CGDirectDisplayID) -> u8;
        pub fn CGBeginDisplayConfiguration(config: *mut *mut c_void) -> i32;
        pub fn CGConfigureDisplayOrigin(
            config: *mut c_void,
            display: CGDirectDisplayID,
            x: i32,
            y: i32,
        ) -> i32;
        pub fn CGConfigureDisplayMirrorOfDisplay(
            config: *mut c_void,
            display: CGDirectDisplayID,
            master: CGDirectDisplayID,
        ) -> i32;
        pub fn CGCompleteDisplayConfiguration(config: *mut c_void, option: u32) -> i32;
    }

    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const i8) -> *mut c_void;
        fn sel_registerName(name: *const i8) -> *mut c_void;
        fn objc_msgSend();
        fn objc_release(obj: *mut c_void);
    }

    // Typed function pointers for objc_msgSend (ARM64 compatible)
    type MsgSend0 = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
    type MsgSend1Ptr = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void;
    type MsgSend1U32 = unsafe extern "C" fn(*mut c_void, *mut c_void, u32) -> *mut c_void;
    type MsgSendSize = unsafe extern "C" fn(*mut c_void, *mut c_void, CGSize) -> *mut c_void;
    type MsgSendMode =
        unsafe extern "C" fn(*mut c_void, *mut c_void, usize, usize, f64) -> *mut c_void;

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct CGSize {
        width: f64,
        height: f64,
    }

    const KCG_CONFIGURE_FOR_APP_ONLY: u32 = 0;
    const KCG_NULL_DIRECT_DISPLAY: CGDirectDisplayID = 0;

    fn djb2_hash(s: &str) -> u32 {
        let mut hash: u64 = 5381;
        for c in s.bytes() {
            hash = ((hash << 5).wrapping_add(hash)).wrapping_add(c as u64);
        }
        (hash & 0xFFFFFFFF) as u32
    }

    fn clamp<T: Ord>(value: T, min: T, max: T) -> T {
        if value < min {
            min
        } else if value > max {
            max
        } else {
            value
        }
    }

    fn get_class(name: &str) -> *mut c_void {
        let name_cstr = CString::new(name).unwrap();
        unsafe { objc_getClass(name_cstr.as_ptr()) }
    }

    fn get_selector(name: &str) -> *mut c_void {
        let name_cstr = CString::new(name).unwrap();
        unsafe { sel_registerName(name_cstr.as_ptr()) }
    }

    // Safe typed message send functions
    #[inline]
    unsafe fn msg_send0(obj: *mut c_void, sel: *mut c_void) -> *mut c_void {
        let f: MsgSend0 = std::mem::transmute(objc_msgSend as *const ());
        f(obj, sel)
    }

    #[inline]
    unsafe fn msg_send1_ptr(obj: *mut c_void, sel: *mut c_void, arg: *mut c_void) -> *mut c_void {
        let f: MsgSend1Ptr = std::mem::transmute(objc_msgSend as *const ());
        f(obj, sel, arg)
    }

    #[inline]
    unsafe fn msg_send1_u32(obj: *mut c_void, sel: *mut c_void, arg: u32) -> *mut c_void {
        let f: MsgSend1U32 = std::mem::transmute(objc_msgSend as *const ());
        f(obj, sel, arg)
    }

    #[inline]
    unsafe fn msg_send_size(obj: *mut c_void, sel: *mut c_void, size: CGSize) -> *mut c_void {
        let f: MsgSendSize = std::mem::transmute(objc_msgSend as *const ());
        f(obj, sel, size)
    }

    #[inline]
    unsafe fn msg_send_mode(
        obj: *mut c_void,
        sel: *mut c_void,
        w: usize,
        h: usize,
        rate: f64,
    ) -> *mut c_void {
        let f: MsgSendMode = std::mem::transmute(objc_msgSend as *const ());
        f(obj, sel, w, h, rate)
    }

    struct VirtualDisplayState {
        display: *mut c_void,
        descriptor: *mut c_void,
        settings: *mut c_void,
        display_id: CGDirectDisplayID,
    }

    impl VirtualDisplayState {
        fn new() -> Self {
            Self {
                display: ptr::null_mut(),
                descriptor: ptr::null_mut(),
                settings: ptr::null_mut(),
                display_id: 0,
            }
        }

        fn is_created(&self) -> bool {
            !self.display.is_null()
        }

        fn get_display_id(&self) -> CGDirectDisplayID {
            self.display_id
        }

        fn create(
            &mut self,
            width: u32,
            height: u32,
            refresh_rate: u32,
            hidpi: bool,
            name: &str,
            ppi: i32,
            use_mirror: bool,
            all_modes: &[(u32, u32)], // All available resolution modes
        ) -> Result<CGDirectDisplayID, String> {
            if self.is_created() {
                self.destroy();
            }

            if width == 0 || height == 0 {
                return Err("Width and height must be greater than 0".to_string());
            }

            let refresh_rate_f64 = clamp(refresh_rate as i32, 30, 120) as f64;
            let ppi = clamp(ppi, 72, 300);

            let descriptor_class = get_class("CGVirtualDisplayDescriptor");
            let display_class = get_class("CGVirtualDisplay");
            let settings_class = get_class("CGVirtualDisplaySettings");
            let mode_class = get_class("CGVirtualDisplayMode");
            let nsstring_class = get_class("NSString");
            let nsarray_class = get_class("NSArray");

            if descriptor_class.is_null()
                || display_class.is_null()
                || settings_class.is_null()
                || mode_class.is_null()
            {
                return Err(
                    "Failed to get CGVirtualDisplay classes - private API may be unavailable"
                        .to_string(),
                );
            }

            let main_display = unsafe { CGMainDisplayID() };
            tracing::info!("[VD] Previous Main display ID: {}", main_display);

            unsafe {
                let alloc_sel = get_selector("alloc");
                let init_sel = get_selector("init");
                let set_name_sel = get_selector("setName:");
                let set_max_pixels_wide_sel = get_selector("setMaxPixelsWide:");
                let set_max_pixels_high_sel = get_selector("setMaxPixelsHigh:");
                let set_size_mm_sel = get_selector("setSizeInMillimeters:");
                let set_serial_sel = get_selector("setSerialNum:");
                let set_product_id_sel = get_selector("setProductID:");
                let set_vendor_id_sel = get_selector("setVendorID:");
                let init_with_descriptor_sel = get_selector("initWithDescriptor:");
                let display_id_sel = get_selector("displayID");
                let set_hidpi_sel = get_selector("setHiDPI:");
                let init_mode_sel = get_selector("initWithWidth:height:refreshRate:");
                let set_modes_sel = get_selector("setModes:");
                let apply_settings_sel = get_selector("applySettings:");
                let string_with_utf8_sel = get_selector("stringWithUTF8String:");
                let array_with_objects_sel = get_selector("arrayWithObjects:count:");

                // Create descriptor
                let descriptor_alloc = msg_send0(descriptor_class, alloc_sel);
                self.descriptor = msg_send0(descriptor_alloc, init_sel);

                if self.descriptor.is_null() {
                    return Err("Failed to create CGVirtualDisplayDescriptor".to_string());
                }

                // Create NSString for name
                let name_cstr = CString::new(name).unwrap();
                let ns_name = msg_send1_ptr(nsstring_class, string_with_utf8_sel, name_cstr.as_ptr() as *mut c_void);

                // Calculate max resolution from all modes
                let max_width = all_modes.iter().map(|(w, _)| *w).max().unwrap_or(width).max(width);
                let max_height = all_modes.iter().map(|(_, h)| *h).max().unwrap_or(height).max(height);

                // Set descriptor properties with MAX resolution to support all modes
                msg_send1_ptr(self.descriptor, set_name_sel, ns_name);
                msg_send1_u32(self.descriptor, set_max_pixels_wide_sel, max_width);
                msg_send1_u32(self.descriptor, set_max_pixels_high_sel, max_height);

                let ratio = 25.4 / ppi as f64;
                let size = CGSize {
                    width: max_width as f64 * ratio,
                    height: max_height as f64 * ratio,
                };
                msg_send_size(self.descriptor, set_size_mm_sel, size);

                let hash = djb2_hash(name);
                msg_send1_u32(self.descriptor, set_serial_sel, hash);
                msg_send1_u32(self.descriptor, set_product_id_sel, (hash >> 16) & 0xFFFF);
                msg_send1_u32(self.descriptor, set_vendor_id_sel, 0xeeee);

                // Create display
                let display_alloc = msg_send0(display_class, alloc_sel);
                self.display = msg_send1_ptr(display_alloc, init_with_descriptor_sel, self.descriptor);

                if self.display.is_null() {
                    objc_release(self.descriptor);
                    self.descriptor = ptr::null_mut();
                    return Err("Failed to create CGVirtualDisplay".to_string());
                }

                // Get display ID
                self.display_id = msg_send0(self.display, display_id_sel) as CGDirectDisplayID;

                if self.display_id == 0 {
                    self.destroy();
                    return Err("Failed to create virtual display (displayID is 0)".to_string());
                }

                // Create settings
                let settings_alloc = msg_send0(settings_class, alloc_sel);
                self.settings = msg_send0(settings_alloc, init_sel);

                let hidpi_value: u32 = if hidpi { 1 } else { 0 };
                msg_send1_u32(self.settings, set_hidpi_sel, hidpi_value);

                // Create ALL modes from available resolutions
                type MsgSendArray = unsafe extern "C" fn(
                    *mut c_void,
                    *mut c_void,
                    *const *mut c_void,
                    usize,
                ) -> *mut c_void;

                // Collect unique resolutions (current + all_modes)
                let mut unique_modes: Vec<(u32, u32)> = vec![(width, height)];
                for &(w, h) in all_modes {
                    if !unique_modes.contains(&(w, h)) {
                        unique_modes.push((w, h));
                    }
                }

                // Create mode objects for all resolutions
                let mut mode_objects: Vec<*mut c_void> = Vec::with_capacity(unique_modes.len() * 2);

                for &(w, h) in &unique_modes {
                    // Main mode
                    let mode_alloc = msg_send0(mode_class, alloc_sel);
                    let mode = msg_send_mode(
                        mode_alloc,
                        init_mode_sel,
                        w as usize,
                        h as usize,
                        refresh_rate_f64,
                    );
                    mode_objects.push(mode);

                    // HiDPI scaled mode (half resolution)
                    if hidpi && w >= 1280 && h >= 720 {
                        let low_mode_alloc = msg_send0(mode_class, alloc_sel);
                        let low_mode = msg_send_mode(
                            low_mode_alloc,
                            init_mode_sel,
                            (w / 2) as usize,
                            (h / 2) as usize,
                            refresh_rate_f64,
                        );
                        mode_objects.push(low_mode);
                    }
                }

                tracing::info!("[VD] Creating display with {} resolution modes", mode_objects.len());

                let f: MsgSendArray = std::mem::transmute(objc_msgSend as *const ());
                let modes_array = f(
                    nsarray_class,
                    array_with_objects_sel,
                    mode_objects.as_ptr(),
                    mode_objects.len(),
                );

                msg_send1_ptr(self.settings, set_modes_sel, modes_array);
                msg_send1_ptr(self.display, apply_settings_sel, self.settings);

                // Post-processing: fix display configuration
                let new_main_display = CGMainDisplayID();
                tracing::info!("[VD] Current Main Display after creation: {}", new_main_display);

                let mut config: *mut c_void = ptr::null_mut();
                CGBeginDisplayConfiguration(&mut config);

                if new_main_display == self.display_id && new_main_display != main_display {
                    tracing::info!("[VD] Restoring primary display as main");
                    CGConfigureDisplayOrigin(config, main_display, 0, 0);
                }

                let mirror_source = CGDisplayMirrorsDisplay(main_display);
                if mirror_source == self.display_id {
                    tracing::info!("[VD] Disabling unwanted mirror mode");
                    CGConfigureDisplayMirrorOfDisplay(config, mirror_source, KCG_NULL_DIRECT_DISPLAY);
                }

                CGCompleteDisplayConfiguration(config, KCG_CONFIGURE_FOR_APP_ONLY);

                let is_mirror = CGDisplayIsInMirrorSet(self.display_id);
                tracing::info!("[VD] Virtual Display is in mirror set: {}", is_mirror);

                CGBeginDisplayConfiguration(&mut config);
                if use_mirror {
                    if is_mirror == 0 {
                        tracing::info!("[VD] Enabling mirror mode");
                        CGConfigureDisplayMirrorOfDisplay(config, self.display_id, main_display);
                    }
                } else if is_mirror == 1 {
                    tracing::info!("[VD] Disabling mirror mode");
                    CGConfigureDisplayMirrorOfDisplay(config, self.display_id, KCG_NULL_DIRECT_DISPLAY);
                }
                CGCompleteDisplayConfiguration(config, KCG_CONFIGURE_FOR_APP_ONLY);

                tracing::info!("[VD] Virtual display created with ID: {}", self.display_id);
            }

            Ok(self.display_id)
        }

        fn destroy(&mut self) {
            if self.display.is_null() {
                return;
            }

            tracing::info!("[VD] Destroying virtual display");

            unsafe {
                if !self.settings.is_null() {
                    objc_release(self.settings);
                    self.settings = ptr::null_mut();
                }
                if !self.descriptor.is_null() {
                    objc_release(self.descriptor);
                    self.descriptor = ptr::null_mut();
                }
                if !self.display.is_null() {
                    objc_release(self.display);
                    self.display = ptr::null_mut();
                }
            }
            self.display_id = 0;
        }
    }

    impl Drop for VirtualDisplayState {
        fn drop(&mut self) {
            self.destroy();
        }
    }

    thread_local! {
        static DISPLAY_STATE: RefCell<VirtualDisplayState> = RefCell::new(VirtualDisplayState::new());
    }

    pub fn create_display(
        width: u32,
        height: u32,
        refresh_rate: u32,
        hidpi: bool,
        name: &str,
        ppi: i32,
        use_mirror: bool,
        all_modes: &[(u32, u32)], // All available resolution modes
    ) -> Result<CGDirectDisplayID, String> {
        DISPLAY_STATE.with(|state| {
            state
                .borrow_mut()
                .create(width, height, refresh_rate, hidpi, name, ppi, use_mirror, all_modes)
        })
    }

    pub fn destroy_display() -> bool {
        DISPLAY_STATE.with(|state| {
            let mut s = state.borrow_mut();
            if s.is_created() {
                s.destroy();
                true
            } else {
                false
            }
        })
    }

    pub fn is_display_created() -> bool {
        DISPLAY_STATE.with(|state| state.borrow().is_created())
    }

    #[allow(dead_code)]
    pub fn get_display_id() -> CGDirectDisplayID {
        DISPLAY_STATE.with(|state| state.borrow().get_display_id())
    }
}

pub struct VirtualDisplay {
    config: DisplayConfig,
    display_id: Option<u32>,
}

impl VirtualDisplay {
    pub fn new(config: DisplayConfig) -> Self {
        Self {
            config,
            display_id: None,
        }
    }

    #[cfg(target_os = "macos")]
    pub fn create(&mut self) -> Result<()> {
        if macos::is_display_created() {
            warn!("Display already created");
            return Ok(());
        }

        info!(
            "Creating virtual display: {}x{} @ {}Hz ({}) with {} modes",
            self.config.width, self.config.height, self.config.frame_rate,
            self.config.name, self.config.available_modes.len()
        );

        // Convert available modes to tuple format
        let all_modes: Vec<(u32, u32)> = self.config.available_modes
            .iter()
            .map(|m| (m.width, m.height))
            .collect();

        match macos::create_display(
            self.config.width,
            self.config.height,
            self.config.frame_rate,
            false,
            &self.config.name,
            81,
            false,
            &all_modes,
        ) {
            Ok(id) => {
                self.display_id = Some(id);
                info!("Virtual display created with ID: {}", id);
                Ok(())
            }
            Err(e) => Err(anyhow!("Failed to create virtual display: {}", e)),
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn create(&mut self) -> Result<()> {
        Err(anyhow!("Virtual displays are only supported on macOS"))
    }

    #[cfg(target_os = "macos")]
    pub fn destroy(&mut self) -> Result<()> {
        if !macos::is_display_created() {
            warn!("No display to destroy");
            return Ok(());
        }

        info!("Destroying virtual display");

        if macos::destroy_display() {
            self.display_id = None;
            info!("Virtual display destroyed");
            Ok(())
        } else {
            Err(anyhow!("Failed to destroy virtual display"))
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn destroy(&mut self) -> Result<()> {
        Err(anyhow!("Virtual displays are only supported on macOS"))
    }
}

impl Drop for VirtualDisplay {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        if macos::is_display_created() {
            let _ = self.destroy();
        }
    }
}
