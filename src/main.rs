mod gamebanana;
mod modmanagement;

use std::{fs};
use std::cmp::PartialEq;
use std::collections::HashMap;
use std::path::Path;
use crate::gamebanana::GbModDownload;
use eframe::{egui, Frame};
use egui::{Context, DroppedFile, FontId, Layout, TextBuffer, TextStyle, vec2};
use egui::Align::{Center, Min};
use egui::FontFamily::{Monospace, Proportional};
use keyvalues_parser::Vdf;
use serde::{Deserialize, Serialize};
use egui_extras::{TableBuilder, Column};
use crate::modmanagement::get_diva_mods_folder;

const ModsFolder: &str = "~/.local/share/Steam/steamapps/common/Hatsune Miku Project DIVA Mega Mix Plus/mods";
const SteamFolder: &str = "/.local/share/Steam/config/libraryfolders.vdf";
const MegaMixAppID: &str = "1761390";
const DivaModFolderSuffix: &str = "/steamapps/common/Hatsune Miku Project DIVA Mega Mix Plus/mods";

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
    downloads: Vec<GbModDownload>,
    mods_directory: String,
    loaded: bool,
    dropped_files: Vec<DroppedFile>
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
            loaded: false,
            mods_directory: ModsFolder.parse().unwrap(),
            dropped_files: Vec::new(),
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
            get_diva_mods_folder(self).expect("Shit, something no worky");
            // load the mod folder now that we've hopefully found it
            self.mods = load_mods(self);
            // set loaded so this doesn't get called 20 billion times.
            self.loaded = true;
        }
        // egui::SidePanel::left("side").show(ctx, |ui| {
        //
        // }) ;
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
            let mut style = (*ctx.style()).clone();
            // style.visuals.code_bg_color
            style.text_styles = [
                (TextStyle::Heading, FontId::new(25.0, Proportional)),
                (TextStyle::Body, FontId::new(24.0, Proportional)),
                (TextStyle::Monospace, FontId::new(24.0, Monospace)),
                (TextStyle::Button, FontId::new(24.0, Proportional)),
                (TextStyle::Small, FontId::new(16.0, Proportional)),
            ].into();
            ctx.set_style(style);
            // ui.with_layout(egui::Layout::top_down_justified(Center), |ui| {
            ui.heading("Rust4Diva Mod Manager");
            // lui.horizontal(|hui| {
            // lui.with_layout(egui::Layout::top_down_justified(Center), |sui| {
            ui.horizontal(|hui| {
                if hui.button("Reload").clicked() {
                    self.mods = load_mods(self);
                    self.loaded = true;
                }
                if hui.button("Save").clicked() {
                    save_mod_configs(self);
                    self.loaded = true;
                }
            });
            ui.style_mut().spacing.item_spacing = vec2(16.0, 16.0);
            // sui.style_mut().text_styles
            TableBuilder::new(ui).resizable(true).striped(false).auto_shrink(false)
                .columns(Column::initial(300.0).at_least(300.0), 2)
                .columns(Column::initial(300.0).at_least(300.0), 1)
                .columns(Column::initial(100.0), 3)
                .header(60.0, |mut header| {
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
                            ui.with_layout(Layout::top_down_justified(Min),|ui| {
                                ui.label(&config.name);
                            });
                        });
                        row.col(|ui| {
                            ui.label(&config.author);
                        });
                        row.col(|ui| {
                            ui.label(&config.description);
                        });
                        row.col(|ui| {
                            ui.label(&config.version);
                        });
                        row.col(|ui| {
                            ui.label(if config.enabled { "True" } else { "False" });
                        });
                        row.col(|ui| {
                            if ui.button("Toggle").clicked() {
                                config.set_enabled(!config.enabled);
                                save_mod_config(mod_path, config);
                            }
                        });
                    });
                });
        });

        // });
    }
}

fn load_mods(diva_data: &mut DivaData) -> Vec<DivaMod> {
    println!("Loading mods from {}", diva_data.clone().mods_directory);
    let mut mods: Vec<DivaMod> = Vec::new();

    if !Path::new(diva_data.clone().mods_directory.as_str()).exists() {
        println!("unable to load mods from nonexistant mods folder");
        return mods;
    }

    let paths = fs::read_dir(diva_data.clone().mods_directory).unwrap();
    for path in paths {
        let mod_path = path.unwrap().path().clone();
        if mod_path.clone().is_file() || !mod_path.clone().is_dir() {
            continue;
        }
        let mut mod_config: DivaModConfig = toml::from_str(fs::read_to_string(mod_path.clone().display().to_string() + "/config.toml").unwrap().as_str()).unwrap();
        // println!("Mod: {}, {}", mod_config.clone().name, mod_config.description.escape_default().to_string());
        mod_config.description = mod_config.description.escape_default().to_string();
        mods.push(DivaMod {
            config: mod_config,
            path: (mod_path.clone().display().to_string() + "/config.toml").to_string(),
        });
    }
    mods
}

fn save_mod_configs(mut diva_data: &mut DivaData) {
    for diva_mod in &diva_data.mods {
        println!("{}\n{}", &diva_mod.path, toml::to_string(&diva_mod.config).unwrap());
    }
}
fn save_mod_config(path: &str, diva_mod_config: &mut DivaModConfig) {
    println!("{}\n{}", path, toml::to_string(&diva_mod_config).unwrap());
}