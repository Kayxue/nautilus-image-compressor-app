use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result};
use gtk::{
    Align, Box, Button, HeaderBar, Label, Orientation, ProgressBar, Spinner, Stack,
    StackTransitionType, Window,
    gio::prelude::ApplicationExt,
    prelude::{BoxExt, ButtonExt, GtkWindowExt, OrientableExt, WidgetExt},
};
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, RelmWidgetExt,
    SimpleComponent, component, main_application,
};

use crate::compress_mode::CompressMode;
use crate::window::window_body::BodyModel;
use crate::{
    output::{OutputMode, save_image},
    window::window_body::BodyOutput,
};

mod window_body;

pub struct Initializer {
    pub paths: Vec<PathBuf>,
    /// XID of the parent (Nautilus) window passed by the extension (X11 only).
    pub parent_xid: Option<u64>,
    /// Exported handle token of the parent (Nautilus) window (Wayland XDG Foreign).
    pub parent_handle: Option<String>,
}

pub struct Config {
    pub compress_mode: CompressMode,
    pub output_mode: OutputMode,
}

struct HeaderModel;

#[derive(Debug)]
enum HeaderOutput {
    Cancel,
    Proceed,
}

#[component(pub)]
impl SimpleComponent for HeaderModel {
    type Init = ();
    type Input = ();
    type Output = HeaderOutput;

    view! {
        #[root]
        HeaderBar {
            set_show_title_buttons: false,
            #[wrap(Some)]
            set_title_widget = &Label {
                set_label: "Compress Images",
                add_css_class: "title",
            },
            pack_start = &Button {
                set_label: "Cancel",
                connect_clicked[sender] => move |_|{
                    sender.output(HeaderOutput::Cancel).unwrap()
                },
            },
            pack_end = &Button {
                set_label: "Compress",
                add_css_class: "suggested-action",
                connect_clicked[sender] => move |_|{
                    sender.output(HeaderOutput::Proceed).unwrap()
                },
            },
        }
    }

    fn init(
        _params: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = HeaderModel;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }
}

pub struct AppModel {
    config: Config,
    header: Controller<HeaderModel>,
    paths: Vec<PathBuf>,
    is_processing: bool,
    processed_count: usize,
    total_count: usize,
    body: Controller<BodyModel>,
}

#[derive(Debug)]
pub enum AppInput {
    Cancel,
    Execute,
    UpdateCompressMode(CompressMode),
    UpdateOutputMode(OutputMode),
    ProgressStep(usize),
    Finished,
}

impl From<BodyOutput> for AppInput {
    fn from(output: BodyOutput) -> Self {
        match output {
            BodyOutput::UpdateConfig(config) => AppInput::UpdateCompressMode(config),
            BodyOutput::UpdateOutputMode(mode) => AppInput::UpdateOutputMode(mode),
        }
    }
}

#[component(pub)]
impl SimpleComponent for AppModel {
    type Init = Initializer;

    type Input = AppInput;
    type Output = ();

    view! {
        #[root]
        Window {
            set_title: Some("Compress Images"),
            set_titlebar: Some(model.header.widget()),
            set_resizable: false,
            set_default_width: 440,

            #[name(stack)]
            Stack {
                set_transition_type: StackTransitionType::Crossfade,

                if model.is_processing {
                    Box {
                        set_orientation: Orientation::Vertical,
                        set_spacing: 16,
                        set_margin_all: 24,
                        set_hexpand: true,
                        set_vexpand: true,
                        set_valign: Align::Center,

                        #[name(spinner)]
                        Spinner {
                            #[watch]
                            set_spinning: model.is_processing,
                            set_size_request: (40, 40),
                            set_halign: Align::Center,
                        },

                        Label {
                            set_label: "Processing......",
                            set_halign: Align::Center,
                            add_css_class: "title-3",
                        },

                        #[name(progress_bar)]
                        ProgressBar {
                            set_hexpand: true,
                            set_show_text: true,
                            #[watch]
                            set_fraction: if model.total_count > 0 {
                                model.processed_count as f64 / model.total_count as f64
                            } else {
                                0.0
                            },
                            #[watch]
                            set_text: Some(&format!("{}/{}", model.processed_count, model.total_count)),
                        },
                    }
                } else {
                    #[name(dialog_vbox1)]
                    Box {
                        set_orientation: Orientation::Vertical,
                        set_spacing: 6,
                        set_margin_all: 12,
                        set_hexpand: true,
                        set_vexpand: true,

                        append: model.body.widget(),
                    }
                },
            }
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let header: Controller<HeaderModel> = HeaderModel::builder()
            .launch(())
            .forward(sender.input_sender(), |msg| match msg {
                HeaderOutput::Cancel => AppInput::Cancel,
                HeaderOutput::Proceed => AppInput::Execute,
            });

        let body = BodyModel::builder()
            .launch(())
            .forward(sender.input_sender(), AppInput::from);

        let model = AppModel {
            config: Config {
                compress_mode: CompressMode::Balanced,
                output_mode: OutputMode::NewFile(".compressed".into()),
            },
            header,
            total_count: init.paths.len(),
            paths: init.paths,
            is_processing: false,
            processed_count: 0,
            body,
        };

        let widgets = view_output!();

        // ── Window Presentation: Wayland XDG Foreign or X11 Transient ─────────
        if let Some(parent_handle) = init.parent_handle {
            bind_transient_to_wayland_parent(&root, parent_handle);
        } else if let Some(parent_xid) = init.parent_xid {
            crate::x11::bind_transient_to_parent(&root, parent_xid);
        }

        // Close on Escape key press
        let key_controller = gtk::EventControllerKey::new();
        key_controller.connect_key_pressed({
            let sender = sender.clone();
            move |_, key, _, _| {
                if key == gtk::gdk::Key::Escape {
                    sender.input(AppInput::Cancel);
                    gtk::glib::Propagation::Stop
                } else {
                    gtk::glib::Propagation::Proceed
                }
            }
        });
        root.add_controller(key_controller);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            AppInput::Cancel => {
                if !self.is_processing {
                    main_application().quit();
                }
            }
            AppInput::Execute => {
                if self.is_processing {
                    return;
                }
                self.is_processing = true;

                // Disable header buttons while processing
                self.header.widget().set_sensitive(false);

                let compress_mode = self.config.compress_mode;
                let paths = self.paths.clone();
                let output_mode = self.config.output_mode.clone();
                let input_sender = sender.input_sender().clone();

                std::thread::spawn(move || {
                    use rayon::prelude::*;

                    let completed_counter = AtomicUsize::new(0);

                    paths.into_par_iter().for_each(|path| {
                        if let Err(e) = process_single_image(&path, &output_mode, compress_mode) {
                            eprintln!(
                                "[nautilus-image-compressor] Error processing {:?}: {:?}",
                                path, e
                            );
                        }
                        let completed = completed_counter.fetch_add(1, Ordering::Relaxed) + 1;
                        let _ = input_sender.send(AppInput::ProgressStep(completed));
                    });

                    // Brief pause so the user sees 100% completion
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    let _ = input_sender.send(AppInput::Finished);
                });
            }
            AppInput::ProgressStep(count) => {
                self.processed_count = count;
            }
            AppInput::Finished => {
                main_application().quit();
            }
            AppInput::UpdateCompressMode(config) => {
                self.config.compress_mode = config;
            }
            AppInput::UpdateOutputMode(mode) => {
                self.config.output_mode = mode;
            }
        }
    }
}

// ── Image processing helpers ──────────────────────────────────────────────────

pub(crate) fn process_single_image(
    path: &Path,
    output_mode: &OutputMode,
    compress_mode: CompressMode,
) -> Result<()> {
    use image::ImageReader;

    let reader = ImageReader::open(path)
        .with_context(|| format!("Failed to open image file: {:?}", path))?
        .with_guessed_format()
        .with_context(|| format!("Failed to guess image format: {:?}", path))?;

    let guessed_format = reader.format();
    let img = reader
        .decode()
        .with_context(|| format!("Failed to decode image: {:?}", path))?;

    let dest = output_mode.destination(path, None);

    save_image(&img, &dest, compress_mode, guessed_format)?;
    println!("[nautilus-image-compressor] Saved: {:?}", dest);

    // Only reached after a successful save, so the original is never lost.
    if output_mode.removes_original(path, &dest) {
        std::fs::remove_file(path)
            .with_context(|| format!("Failed to remove original file: {:?}", path))?;
        println!("[nautilus-image-compressor] Removed original: {:?}", path);
    }

    Ok(())
}

/// Binds `window` as a transient child of an exported Wayland parent surface
/// using the XDG Foreign protocol (zxdg_importer_v2).
fn bind_transient_to_wayland_parent(window: &Window, parent_handle: String) {
    use gdk4_wayland::{WaylandToplevel, prelude::*};
    use gtk::prelude::NativeExt;

    window.connect_realize(move |win| {
        let surface = match win.surface() {
            Some(s) => s,
            None => {
                eprintln!("[nautilus-image-converter] Could not obtain window surface on realize.");
                return;
            }
        };

        match surface.downcast::<WaylandToplevel>() {
            Ok(toplevel) => {
                let ok = toplevel.set_transient_for_exported(&parent_handle);
                if ok {
                    println!(
                        "[nautilus-image-converter] Successfully bound Wayland transient-for to exported handle: {}",
                        parent_handle
                    );
                } else {
                    eprintln!(
                        "[nautilus-image-converter] Failed to bind Wayland transient-for to handle: {}",
                        parent_handle
                    );
                }
            }
            Err(_) => {
                eprintln!(
                    "[nautilus-image-converter] Window surface is not a Wayland toplevel (running on X11?)."
                );
            }
        }
    });
}
