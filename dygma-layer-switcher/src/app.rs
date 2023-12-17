use crate::helpers::remove_opt_index;
use crate::structs::*;
use crate::templates::*;
use crate::{layer, verbiage};
use crossbeam_channel::{Receiver, Sender};
use dygma_focus::Focus;
use eframe::egui::{
    CentralPanel, CollapsingHeader, Context, DragValue, ScrollArea, TopBottomPanel,
};
use eframe::{egui, Frame, Storage};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use tracing::{error, trace, warn};
use tray_icon::menu::MenuEvent;
use tray_icon::{ClickType, TrayIconEvent};

const MAX_LAYERS: u8 = 10;

lazy_static! {
    pub static ref CHANNELS: (Sender<AppDetails>, Receiver<AppDetails>) =
        crossbeam_channel::unbounded::<AppDetails>();
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct DygmaLayerSwitcher {
    pub logging: bool,
    pub port: String,
    pub base_layer: u8,
    pub mappings: BTreeMap<u8, Layer>,
    pub hidden_layers: BTreeSet<u8>,

    #[serde(skip)]
    pub editing_port: bool,

    #[serde(skip)]
    pub remove_app: Option<usize>,

    #[serde(skip)]
    pub remove_exclude: Option<usize>,

    #[serde(skip)]
    pub remove_hidden_layer: Option<u8>,

    #[serde(skip)]
    pub window_visible: bool,
}

impl Default for DygmaLayerSwitcher {
    fn default() -> Self {
        let focus = Focus::default();
        let port = focus.find_first().unwrap_or_else(|_| {
            error!("{}", verbiage::NO_KEYBOARD_MESSAGE);
            std::process::exit(1);
        });

        Self {
            logging: false,
            port: port.port,
            base_layer: 1,
            mappings: (0..MAX_LAYERS)
                .map(|i| (i, Layer::new(i)))
                .collect::<BTreeMap<u8, Layer>>(),
            hidden_layers: BTreeSet::new(),

            editing_port: false,
            remove_app: None,
            remove_exclude: None,
            remove_hidden_layer: None,
            window_visible: true,
        }
    }
}

impl DygmaLayerSwitcher {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }

    fn logging_control(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(verbiage::LOGGING_SETTING_HEADING)
                .on_hover_text(verbiage::LOGGING_SETTING_HINT);
            ui.checkbox(&mut self.logging, "")
                .on_hover_text(verbiage::LOGGING_SETTING_HINT);
        });
    }

    fn port_control(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(verbiage::PORT_SETTING_HEADING)
                .on_hover_text(verbiage::PORT_SETTING_HINT);
            if ui
                .button(verbiage::PORT_SETTING_REFRESH_HEADING)
                .on_hover_text(verbiage::PORT_SETTING_REFRESH_HINT)
                .clicked()
            {
                let focus = Focus::default();
                match focus.find_first() {
                    Ok(port) => self.port = port.port,
                    Err(_) => warn!("{}", verbiage::NO_KEYBOARD_MESSAGE),
                }
            };
            editable_label(ui, &mut self.port, &mut self.editing_port);
        });
    }

    fn base_layer_control(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(verbiage::BASE_LAYER_SETTING_HEADING)
                .on_hover_text(verbiage::BASE_LAYER_SETTING_HINT);
            ui.add(DragValue::new(&mut self.base_layer).clamp_range(1..=MAX_LAYERS))
                .on_hover_text(verbiage::BASE_LAYER_VALUE_HINT);
        });
    }

    fn hidden_layer_control(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if !self.hidden_layers.is_empty() {
                CollapsingHeader::new(verbiage::HIDDEN_LAYERS_HEADING)
                    .default_open(false)
                    .show(ui, |ui| {
                        for layer in self.hidden_layers.iter() {
                            ui.horizontal(|ui| {
                                if ui
                                    .button(verbiage::BUTTON_REMOVE)
                                    .on_hover_text(format!(
                                        "{} {}.",
                                        verbiage::HIDDEN_LAYERS_UNHIDE_HINT,
                                        layer + 1
                                    ))
                                    .clicked()
                                {
                                    self.remove_hidden_layer = Some(*layer);
                                }
                                ui.label(self.mappings[layer].name.as_str())
                                    .on_hover_text(format!("{} {}.", verbiage::LAYER, layer + 1));
                            });
                        }
                        if let Some(layer) = self.remove_hidden_layer {
                            self.hidden_layers.remove(&layer);
                            self.remove_hidden_layer = None;
                        }
                    });
            }
        });
    }

    fn top_panel(&mut self, ctx: &Context) {
        TopBottomPanel::top("top_panel").show(ctx, |ui| {
            CollapsingHeader::new(verbiage::SETTINGS_HEADING)
                .default_open(true)
                .show(ui, |ui| {
                    self.logging_control(ui);
                    self.port_control(ui);
                    self.base_layer_control(ui);
                    self.hidden_layer_control(ui);
                });
        });
    }

    fn central_panel(&mut self, ctx: &Context) {
        CentralPanel::default().show(ctx, |ui| {
            ScrollArea::new([true, true])
                .drag_to_scroll(true)
                .auto_shrink(false)
                .show(ui, |ui| {
                    for (index, layer) in self.mappings.iter_mut() {
                        if self.hidden_layers.contains(index) {
                            continue;
                        }
                        ui.horizontal(|ui| {
                            if ui.button("👁").clicked() {
                                self.hidden_layers.insert(*index);
                            }
                            editable_collapsing(ui, &mut layer.name, &mut layer.is_editing, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{} {}", verbiage::LAYER, index + 1));
                                    if ui
                                        .button(verbiage::BUTTON_ADD_WINDOW)
                                        .on_hover_text(verbiage::WINDOW)
                                        .clicked()
                                    {
                                        layer.apps.push(App::new_window());
                                    }
                                    if ui
                                        .button(verbiage::BUTTON_ADD_PROCESS)
                                        .on_hover_text(verbiage::PROCESS)
                                        .clicked()
                                    {
                                        layer.apps.push(App::new_process());
                                    }
                                    if ui
                                        .button(verbiage::BUTTON_ADD_PARENT)
                                        .on_hover_text(verbiage::PARENT)
                                        .clicked()
                                    {
                                        layer.apps.push(App::new_parent());
                                    }
                                });

                                CollapsingHeader::new(verbiage::MODE_WINDOWS_HEADING)
                                    .default_open(true)
                                    .show(ui, |ui| {
                                        for (index, app) in layer.apps.iter_mut().enumerate() {
                                            if let Mode::Window(window) = &mut app.mode {
                                                ui.horizontal(|ui| {
                                                    ui.checkbox(&mut app.is_enabled, "").on_hover_text(verbiage::CHECKBOX_ACTIVE);
                                                    if ui
                                                        .button(verbiage::BUTTON_REMOVE)
                                                        .on_hover_text(verbiage::MODE_WINDOWS_HINT)
                                                        .clicked()
                                                    {
                                                        self.remove_app = Some(index);
                                                    }
                                                    editable_label(
                                                        ui,
                                                        &mut window.name,
                                                        &mut window.is_editing,
                                                    );
                                                });
                                            }
                                        }
                                    })
                                    .header_response
                                    .on_hover_text(verbiage::WINDOW);

                                CollapsingHeader::new(verbiage::MODE_PROCESSES_HEADING)
                                    .default_open(true)
                                    .show(ui, |ui| {
                                        for (index, app) in layer.apps.iter_mut().enumerate() {
                                            if let Mode::Process(process) = &mut app.mode {
                                                ui.horizontal(|ui| {
                                                    ui.checkbox(&mut app.is_enabled, "").on_hover_text(verbiage::CHECKBOX_ACTIVE);
                                                    if ui
                                                        .button(verbiage::BUTTON_REMOVE)
                                                        .on_hover_text(verbiage::MODE_PROCESSES_HINT)
                                                        .clicked()
                                                    {
                                                        self.remove_app = Some(index);
                                                    }
                                                    editable_label(
                                                        ui,
                                                        &mut process.name,
                                                        &mut process.is_editing,
                                                    );
                                                });
                                            }
                                        }
                                    })
                                    .header_response
                                    .on_hover_text(verbiage::PROCESS);

                                CollapsingHeader::new(verbiage::MODE_PARENT_HEADING)
                                    .default_open(true)
                                    .show(ui, |ui| {
                                        for (index, app) in layer.apps.iter_mut().enumerate() {
                                            if let Mode::Parent(parent) = &mut app.mode {
                                                ui.horizontal(|ui| {
                                                    ui.checkbox(&mut app.is_enabled, "").on_hover_text(verbiage::CHECKBOX_ACTIVE);
                                                    if ui
                                                        .button(verbiage::BUTTON_REMOVE)
                                                        .on_hover_text(verbiage::MODE_PARENT_HINT)
                                                        .clicked()
                                                    {
                                                        self.remove_app = Some(index);
                                                    }
                                                    if ui
                                                        .button(verbiage::BUTTON_ADD_EXCLUDE)
                                                        .on_hover_text(verbiage::EXCLUDES_HINT)
                                                        .clicked()
                                                    {
                                                        parent.excludes.push(Exclude::new());
                                                    }
                                                    editable_label(
                                                        ui,
                                                        &mut parent.name,
                                                        &mut parent.is_editing,
                                                    );
                                                });

                                                if !parent.excludes.is_empty() {
                                                    CollapsingHeader::new(verbiage::MODE_PARENT_EXCLUDES_HEADING)
                                                        .id_source(format!("excludes_{}", index))
                                                        .default_open(true)
                                                        .show(ui, |ui| {
                                                            parent
                                                                .excludes
                                                                .iter_mut()
                                                                .enumerate()
                                                                .for_each(|(index, exclude)| {
                                                                    ui.horizontal(|ui| {
                                                                        ui.checkbox(
                                                                            &mut exclude.is_enabled,
                                                                            "",
                                                                        ).on_hover_text(verbiage::CHECKBOX_ACTIVE);
                                                                        if ui
                                                                        .button(verbiage::BUTTON_REMOVE)
                                                                        .on_hover_text(
                                                                            verbiage::MODE_PARENT_EXCLUDES_HINT,
                                                                        )
                                                                        .clicked()
                                                                    {
                                                                        self.remove_exclude =
                                                                            Some(index);
                                                                    }
                                                                        editable_label(
                                                                            ui,
                                                                            &mut exclude.name,
                                                                            &mut exclude.is_editing,
                                                                        );
                                                                    });
                                                                });
                                                            remove_opt_index(
                                                                &mut parent.excludes,
                                                                &mut self.remove_exclude,
                                                            );
                                                        })
                                                        .header_response
                                                        .on_hover_text(verbiage::EXCLUDES_HINT);
                                                }
                                            }
                                        }
                                    })
                                    .header_response
                                    .on_hover_text(verbiage::PARENT);
                                remove_opt_index(&mut layer.apps, &mut self.remove_app);
                            });
                        });
                    }
                });
        });
    }
}

impl eframe::App for DygmaLayerSwitcher {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        // TODO: Temp fix to run loop while app is out of focus
        ctx.request_repaint();

        // Window focus
        if let Ok(event) = CHANNELS.1.try_recv() {
            // let config = self.clone();
            // thread::spawn(move || layer::process(&config, &event));
            layer::process(self, &event)
        }

        // Tray
        if let Ok(event) = TrayIconEvent::receiver().try_recv() {
            trace!("Tray icon event: {event:?}");
            match event.click_type {
                ClickType::Left => {}
                ClickType::Right => {
                    // Menu
                }
                ClickType::Double => {
                    self.window_visible = !self.window_visible;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(self.window_visible))
                }
            }
        }
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            trace!("Tray menu event: {:?}", event);
            // Exit
            if event.id == "1001" {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        // UI
        self.top_panel(ctx);
        self.central_panel(ctx);
    }

    fn save(&mut self, storage: &mut dyn Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }
}
