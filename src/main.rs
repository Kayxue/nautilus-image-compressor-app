use crate::window::{AppModel, Initializer};
use argh::FromArgs;
use relm4::RelmApp;
use std::path::PathBuf;

mod compress_mode;
mod compressor;
mod output;
mod window;
mod x11;

#[derive(FromArgs)]
/// A image tool that can resize, rotate, and convert images.
struct Args {
    /// paths to the input image files.
    #[argh(positional)]
    paths: Vec<PathBuf>,
    /// XID of the parent (Nautilus) window to set as transient-for (X11 only).
    #[argh(option)]
    parent_xid: Option<u64>,
    /// exported handle token of the parent (Nautilus) window (Wayland XDG Foreign).
    #[argh(option)]
    parent_handle: Option<String>,
}

fn main() {
    let Args {
        paths,
        parent_xid,
        parent_handle,
    } = argh::from_env();

    if paths.is_empty() {
        eprintln!("Error: at least one input path is required.");
        std::process::exit(1);
    }

    if paths.iter().any(|e| !e.is_file()) {
        eprintln!("Error: one or more input paths do not exist or are not files.");
        std::process::exit(1);
    }

    let initializer = Initializer {
        paths,
        parent_xid,
        parent_handle,
    };

    let relm = RelmApp::new("com.kay.nautilus_image_converter").with_args(vec![]);
    relm.run::<AppModel>(initializer);
}
