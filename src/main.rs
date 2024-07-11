use std::cmp::PartialEq;
use std::collections::HashMap;
use std::fs::File;
use eframe::{egui, Frame};
use eframe::emath::Align;
use egui::{Context, DroppedFile, FontId, Layout, TextBuffer, TextStyle, vec2};
use egui::Align::Min;
use egui::FontFamily::{Monospace, Proportional};
use egui_extras::{Column, TableBuilder};
use serde::{Deserialize, Serialize};

use crate::gamebanana::{download_mod_file, fetch_mod_data, GBMod};
use crate::modmanagement::{get_diva_folder, load_diva_ml_config, load_mods, save_mod_config, save_mod_configs, unpack_mod};

mod gamebanana;
mod modmanagement;

const MODS_FOLDER: &str = "~/.local/share/Steam/steamapps/common/Hatsune Miku Project DIVA Mega Mix Plus";
const STEAM_FOLDER: &str = "/.local/share/Steam/config/libraryfolders.vdf";
const MEGA_MIX_APP_ID: &str = "1761390";
const DIVA_MOD_FOLDER_SUFFIX: &str = "/steamapps/common/Hatsune Miku Project DIVA Mega Mix Plus";

#[derive(Clone, Deserialize, Serialize)]
struct DivaModConfig {
    enabled: bool,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    dll: Vec<String>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    author: String,
}

#[derive(Clone)]
struct DivaMod {
    config: DivaModConfig,
    path: String,
}

#[derive(Clone, Deserialize, Serialize)]
struct DivaModLoader {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    console: bool,
    #[serde(default)]
    mods: String,
    #[serde(default)]
    version: String,
    // #[serde(default)]
    // priority: Vec<String>,
}


impl DivaModConfig {
    pub fn set_enabled(&mut self, enabled: bool) -> Self {
        // let change = self.enabled;
        self.enabled = enabled;
        self.to_owned()
    }
}

impl DivaMod {
    pub fn set_enabled(&mut self, enabled: bool) -> Self {
        let config = self.config.set_enabled(enabled);
        self.config = config;
        self.to_owned()
    }
}
#[derive(Clone)]
struct DivaData {
    mods: Vec<DivaMod>,
    current_dl: Option<GBMod>,
    downloads: Vec<GBMod>,
    mods_directory: String,
    diva_directory: String,
    dl_mod_url: String,
    loaded: bool,
    dropped_files: Vec<DroppedFile>,
    show_dl: bool,
    diva_mod_loader: DivaModLoader,
}


#[derive(Deserialize)]
struct LibraryFolders(HashMap<String, String>);


fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 720.0]),
        ..Default::default()
    };
    println!("Starting Rust4Diva");
    eframe::run_native(
        "Rust4Diva",
        options,
        Box::new(|_cc| {
            Ok(Box::<DivaData>::default())
        }),
    )
}


impl Default for DivaData {
    fn default() -> Self {
        // let mut mods = Vec::new();
        Self {
            mods: Vec::new(),
            downloads: Vec::new(),
            current_dl: None,
            loaded: false,
            mods_directory: "".to_string(),
            dropped_files: Vec::new(),
            dl_mod_url: "".to_string(),
            show_dl: false,
            diva_directory: "".to_string(),
            diva_mod_loader: DivaModLoader {
                enabled: false,
                console: false,
                mods: "".to_string(),
                version: "".to_string(),
            },
        }
    }
}


impl PartialEq for DivaMod {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl eframe::App for DivaData {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        if !self.loaded {
            // try to find the mods folder
            self.diva_directory = get_diva_folder().expect("Shit, something no worky");

            // load diva mod loader config
            self.diva_mod_loader = load_diva_ml_config(self.clone().diva_directory).unwrap();
            // load the mod folder now that we've hopefully found it
            self.mods = load_mods(self);
            // set loaded so this doesn't get called 20 billion times.
            self.loaded = true;
        }
        // begin the top bar of the app
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.label("File");
                ui.label("Edit");
                ui.label("View");
                ui.label("Help");
            });
        });
        // begin the actual contents
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.style_mut().spacing.item_spacing = vec2(16.0, 16.0);
            let mut style = (*ctx.style()).clone();
            style.text_styles = [
                (TextStyle::Heading, FontId::new(25.0, Proportional)),
                (TextStyle::Body, FontId::new(24.0, Proportional)),
                (TextStyle::Monospace, FontId::new(24.0, Monospace)),
                (TextStyle::Button, FontId::new(24.0, Proportional)),
                (TextStyle::Small, FontId::new(16.0, Proportional)),
            ].into();
            ctx.set_style(style);
            ui.with_layout(Layout::top_down_justified(Align::Center), |ui| {
                ui.heading("Rust4Diva Mod Manager");
            });
            egui::ScrollArea::vertical().show(ui, |s| {
                TableBuilder::new(s)
                    .max_scroll_height(ctx.input(|i: &egui::InputState| i.screen_rect()).size().y - 220.0)
                    .drag_to_scroll(false)
                    .resizable(true)
                    .columns(Column::initial(300.0).at_least(300.0), 2)
                    .columns(Column::initial(300.0).at_least(300.0), 1)
                    .columns(Column::initial(100.0), 3)
                    .header(20.0, |mut header| {
                        header.col(|ui| {
                            ui.heading("Mod");
                        });
                        header.col(|ui| {
                            ui.heading("Author(s)");
                        });
                        header.col(|ui| {
                            ui.heading("Description");
                        });
                        header.col(|ui| {
                            ui.heading("Version");
                        });
                        header.col(|ui| {
                            ui.heading("Enabled");
                        });
                        header.col(|ui| {
                            ui.heading("Toggle");
                        });
                    })
                    .body(|mut body| {
                        body.rows(40.0, self.mods.len(), |mut row| {
                            let mut divamod = &mut self.mods[row.index()];
                            let mod_path = &divamod.path;
                            let mut config = &mut divamod.config;
                            row.col(|ui| {
                                ui.with_layout(Layout::top_down_justified(Min), |ui| {
                                    ui.label(&config.name);
                                });
                            });
                            row.col(|ui| { ui.label(&config.author); });
                            row.col(|ui| { ui.label(&config.description); });
                            row.col(|ui| { ui.label(&config.version); });
                            row.col(|ui| { ui.label(if config.enabled { "True" } else { "False" }); });
                            row.col(|ui| {
                                if ui.button("Toggle").clicked() {
                                    config.set_enabled(!config.enabled);
                                    save_mod_config(mod_path, config);
                                    println!("{}", ctx.input(|i: &egui::InputState| i.screen_rect()).size().y - 1000.0);
                                }
                            });
                        });
                    });
            });
            ui.horizontal_centered(|hui| {
                if hui.button("Reload").clicked() {
                    self.mods = load_mods(self);
                    self.loaded = true;
                }
                if hui.button("Save").clicked() {
                    save_mod_configs(self);
                    self.loaded = true;
                }
                let dl_mod = hui.button("Download from GB");
                if dl_mod.clicked() {
                    self.show_dl = true;
                }
            });
        });

        if self.clone().show_dl {
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("dl_win"),
                egui::ViewportBuilder::default()
                    .with_title("Download Mod")
                    .with_inner_size([800.0, 600.0]),
                |ctx, class| {
                    assert!(
                        class == egui::ViewportClass::Immediate,
                        "This egui backend doesn't support multiple viewports"
                    );

                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            let dl_mod = ui.label("Download URL: ");
                            let res = ui.text_edit_singleline(&mut self.dl_mod_url);
                            res.labelled_by(dl_mod.id);
                        });
                        if ui.button("Get info").clicked() {
                            println!("{}", self.dl_mod_url);
                            let gb_mod = fetch_mod_data(self.dl_mod_url.as_str());
                            if gb_mod.is_some() {
                                self.downloads.push(gb_mod.unwrap());
                            }
                        }
                        for mut gb_mod in self.clone().downloads.iter() {
                            ui.horizontal(|h| {
                                h.label(&gb_mod.name);
                                h.label("Files: ");
                            });
                            for mut file in gb_mod.files.iter() {
                                ui.horizontal(|h| {
                                    let dll = h.label(format!("{}", file._sFile));
                                    let mut dl = h.button("Download This file").labelled_by(dll.id);
                                    if dl.clicked() {
                                        let res = download_mod_file(file);
                                        match res {
                                            Ok(_) => {
                                                unpack_mod(File::open("/tmp/rust4diva/".to_owned() + &file._sFile).unwrap(), self.clone());
                                                println!("Mod installed, reloading mods");
                                                self.mods = load_mods(self);
                                            }
                                            Err(e) => panic!("Failed to write data: {}", e)
                                        }
                                        // unpack_mod(file, &self);
                                    }
                                });
                            }
                        }
                    });

                    if ctx.input(|i| i.viewport().close_requested()) {
                        // Tell parent viewport that we should not show next frame:
                        self.show_dl = false;
                    }
                })
        }
    }
}



