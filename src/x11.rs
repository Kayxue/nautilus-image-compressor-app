use gdk4_x11::{X11Display, X11Surface, prelude::*};
use gtk::{
    Window,
    prelude::{NativeExt, WidgetExt},
};

// Minimal FFI binding to the one Xlib function we need.
// GDK4 already links against libX11, so this is always available on X11/XWayland.
#[link(name = "X11")]
unsafe extern "C" {
    fn XSetTransientForHint(
        display: *mut std::ffi::c_void,
        w: std::os::raw::c_ulong,
        prop_window: std::os::raw::c_ulong,
    ) -> std::os::raw::c_int;
}

/// Binds `window` as a transient child of the X11 window identified by
/// `parent_xid`.
///
/// The actual `XSetTransientForHint` call is deferred to `connect_realize` so
/// the window's own X11 surface (and its XID) is guaranteed to exist.
///
/// If GDK already tracks the parent surface (same-process scenario) we also
/// watch it via `connect_mapped_notify`: when the parent is unmapped/destroyed
/// we clear the transient hint instead of closing, keeping our window alive.
///
/// On a pure-Wayland session (no XWayland) the display will not be an
/// `X11Display` and this function is a no-op.
pub fn bind_transient_to_parent(window: &Window, parent_xid: u64) {
    // Defer to connect_realize: the window's GDK surface only exists after the
    // underlying OS window is created.
    window.connect_realize(move |win| {
        // Verify the display backend is X11 / XWayland.
        let display = match win.display().downcast::<X11Display>() {
            Ok(d) => d,
            Err(_) => {
                eprintln!(
                    "[nautilus-image-converter] Pure Wayland session detected; \
                     skipping X11 parent-window binding."
                );
                return;
            }
        };

        // Obtain our window's X11 surface and its XID.
        let our_surface = match win.surface().and_then(|s| s.downcast::<X11Surface>().ok()) {
            Some(s) => s,
            None => {
                eprintln!(
                    "[nautilus-image-converter] Could not downcast window surface \
                     to X11Surface."
                );
                return;
            }
        };
        let our_xid = our_surface.xid();

        // Set WM_TRANSIENT_FOR so the window manager stacks us above Nautilus.
        // SAFETY: `display.xdisplay()` is a valid *mut xlib::Display from GDK.
        unsafe { set_transient_for_xlib(&display, our_xid as u64, parent_xid) };

        // If GDK already tracks the parent surface in this process (uncommon in
        // cross-process use, but possible if the extension is ever in-process),
        // watch it for unmapping.  When it becomes unmapped we clear the
        // WM_TRANSIENT_FOR hint so the WM treats our window as a standalone
        // top-level and does not close/hide it along with the parent.
        if let Some(parent_surf) = X11Surface::lookup_for_display(&display, parent_xid) {
            let our_surf_clone = our_surface.clone();
            let display_clone = display.clone();
            parent_surf.connect_mapped_notify(move |surf| {
                if !surf.is_mapped() {
                    // Parent window has gone away — decouple our window.
                    // Pointing WM_TRANSIENT_FOR at the root window is the
                    // ICCCM §4.1.2.6 way to say "no parent".
                    unsafe {
                        set_transient_for_xlib(
                            &display_clone,
                            our_surf_clone.xid() as u64,
                            display_clone.xrootwindow() as u64,
                        )
                    };
                }
            });
        }
    });
}

/// Sets `WM_TRANSIENT_FOR` on `our_xid` pointing at `parent_xid` via Xlib.
///
/// # Safety
/// `display` must be a live GDK X11 display; both XIDs must be valid X11
/// window identifiers.
unsafe fn set_transient_for_xlib(display: &X11Display, our_xid: u64, parent_xid: u64) {
    let xdisplay = unsafe { display.xdisplay() } as *mut std::ffi::c_void;
    unsafe {
        XSetTransientForHint(
            xdisplay,
            our_xid as std::os::raw::c_ulong,
            parent_xid as std::os::raw::c_ulong,
        );
    }
}
