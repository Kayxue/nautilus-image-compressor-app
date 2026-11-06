use crate::{output::OutputMode, window::CompressMode};
use gtk::{
    Box, CheckButton, Entry, Label, Orientation,
    prelude::{BoxExt, CheckButtonExt, EditableExt, ObjectExt, OrientableExt, WidgetExt},
};
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, RelmWidgetExt,
    SimpleComponent, component,
};

/// Output shared by all window bodies.
#[derive(Debug)]
pub enum BodyOutput {
    UpdateConfig(CompressMode),
    UpdateOutputMode(OutputMode),
}

/// Makes `target` sensitive only while `radio` is active.
fn bind_sensitive(radio: &CheckButton, target: &impl gtk::prelude::IsA<gtk::Widget>) {
    radio
        .bind_property("active", target, "sensitive")
        .sync_create()
        .build();
}

// ── Filename section (shared by Resize and Rotate) ───────────────────────────

pub struct FilenameInit {
    /// Default suffix appended to the file title.
    pub suffix: &'static str,
    /// Label of the "overwrite in place" radio button.
    pub in_place_label: &'static str,
}

pub struct FilenameModel;

#[component(pub)]
impl SimpleComponent for FilenameModel {
    type Init = FilenameInit;

    type Input = ();
    type Output = OutputMode;

    view! {
        #[root]
        Box {
            set_orientation: Orientation::Vertical,
            set_spacing: 12,

            Label {
                set_xalign: 0f32,
                set_label: "<b>Filename</b>",
                set_use_markup: true,
            },

            Box {
                set_orientation: Orientation::Vertical,
                set_spacing: 6,
                set_margin_start: 12,

                Box {
                    set_spacing: 6,

                    #[name(append_radiobutton)]
                    CheckButton {
                        set_active: true,
                        set_label: Some("Append"),
                        set_use_underline: true,
                        connect_toggled[sender, name_entry] => move |btn| {
                            if btn.is_active() {
                                sender.output(OutputMode::NewFile(name_entry.text().into())).unwrap();
                            }
                        }
                    },

                    #[name(name_entry)]
                    Entry {
                        set_hexpand: true,
                        set_text: init.suffix,
                        connect_changed[sender] => move |entry| {
                            sender.output(OutputMode::NewFile(entry.text().into())).unwrap();
                        }
                    },

                    Label {
                        set_label: "to file title",
                    }
                },

                CheckButton {
                    set_label: Some(init.in_place_label),
                    set_use_underline: true,
                    set_group: Some(&append_radiobutton),
                    connect_toggled[sender] => move |btn| {
                        if btn.is_active() {
                            sender.output(OutputMode::InPlace).unwrap();
                        }
                    }
                }
            }
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = FilenameModel;
        let widgets = view_output!();
        bind_sensitive(&widgets.append_radiobutton, &widgets.name_entry);
        ComponentParts { model, widgets }
    }
}

/// Launches the filename section and forwards its output through `sender`.
fn launch_filename<C>(init: FilenameInit, sender: &ComponentSender<C>) -> Controller<FilenameModel>
where
    C: Component<Output = BodyOutput>,
{
    FilenameModel::builder()
        .launch(init)
        .forward(sender.output_sender(), BodyOutput::UpdateOutputMode)
}

pub struct BodyModel {
    filename: Controller<FilenameModel>,
}

#[component(pub)]
impl SimpleComponent for BodyModel {
    type Init = ();

    type Input = ();
    type Output = BodyOutput;

    view! {
        #[root]
        Box {
            set_hexpand: true,
            set_vexpand: false,
            set_orientation: Orientation::Vertical,
            set_margin_all: 12,
            set_spacing: 12,

            Label {
                set_xalign: 0f32,
                set_label: "<b>Compression mode</b>",
                set_use_markup: true
            },

            Box {
                set_hexpand: true,
                set_orientation: Orientation::Vertical,
                set_spacing: 6,
                set_margin_start: 12,

                Box {
                    set_spacing: 6,

                    #[name(balance_radiobutton)]
                    CheckButton {
                        set_label: Some("Balance"),
                        set_use_underline: true,
                        set_active: true,
                        connect_toggled[sender] => move |btn| {
                            if btn.is_active() {
                                sender.output(BodyOutput::UpdateConfig(CompressMode::Balanced)).unwrap();
                            }
                        }
                    },
                },

                Box {
                    set_spacing: 6,

                    #[name(ultra_radio_button)]
                    CheckButton {
                        set_label: Some("Ultra"),
                        set_use_underline: true,
                        set_group: Some(&balance_radiobutton),
                        connect_toggled[sender] => move |btn| {
                            if btn.is_active() {
                                sender.output(BodyOutput::UpdateConfig(CompressMode::Ultra)).unwrap();
                            }
                        }
                    },
                },
            },

            append: model.filename.widget(),
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let filename = launch_filename(
            FilenameInit {
                suffix: ".compressed",
                in_place_label: "Compress in place",
            },
            &sender,
        );
        let model = BodyModel { filename };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }
}
