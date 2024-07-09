mod gamebanana;

use std::{fs, path};
use std::cmp::PartialEq;
use std::collections::HashMap;
use std::ops::Index;
use std::path::Path;
use crate::gamebanana::GbModDownload;
use eframe::{egui, Frame};
use egui::{Context, Rangef, TextBuffer, vec2};
use egui::Align::{Center, Min};
use keyvalues_parser::Vdf;
use serde::{Deserialize, Serialize};
use egui_extras::{TableBuilder, Size, Column};

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
        let mut config = self.config.set_enabled(enabled);
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
        Box::new(|cc| {
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
        }
    }
}

impl DivaData {
    fn update_mod(&mut self, index: usize, diva_mod: DivaMod) {
        self.mods[index] = diva_mod;
    }
}

impl PartialEq for DivaMod {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl eframe::App for DivaData {
    fn update(&mut self, ctx: &Context, frame: &mut Frame) {
        if !self.loaded {
            // try to find the mods folder
            get_diva_mods_folder(self);
            // load the mod folder now that we've hopefully found it
            self.mods = load_mods(self);
            // set loaded so this doesn't get called 20 billion times.
            self.loaded = true;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(egui::Layout::top_down_justified(Center), |lui| {
                lui.heading("Rust4Diva Mod Manager");
                // lui.horizontal(|hui| {
                lui.style_mut().spacing.item_spacing = vec2(80.0, 16.0);
                lui.with_layout(egui::Layout::top_down_justified(Min), |sui| {
                    sui.style_mut().spacing.item_spacing = vec2(16.0, 16.0);
                    TableBuilder::new(sui).resizable(true).striped(true).auto_shrink(true)
                        .columns(Column::initial(300.0).at_least(300.0), 3)
                        .columns(Column::auto(), 3)
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
                            for divamod in &mut self.mods {
                                let mut config = &mut divamod.config;
                                body.row(10.0, |mut row| {
                                    row.col(|ui| {
                                        ui.label(&config.name);
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
                                        }
                                    });
                                });
                            }
                        });
                });
                // });
            });
            ui.with_layout(egui::Layout::top_down_justified(Center), |lui| {
                lui.horizontal(|hui| {
                    if hui.button("Reload").clicked() {
                        load_mods(self);
                        self.loaded = true;
                    }
                    if hui.button("Save").clicked() {
                        save_mod_configs(self);
                        self.loaded = true;
                    }
                });
            });
        });
    }
}

fn load_mods(mut diva_data: &mut DivaData) -> Vec<DivaMod> {
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
        let mod_config: DivaModConfig = toml::from_str(fs::read_to_string(mod_path.clone().display().to_string() + "/config.toml").unwrap().as_str()).unwrap();
        println!("Mod: {}", mod_config.clone().name);
        mods.push(DivaMod {
            config: mod_config,
            path: (mod_path.clone().display().to_string() + "/config.toml").to_string(),
        });
    }
    mods
}


fn get_diva_mods_folder(mut diva_data: &mut DivaData) -> Result<(), Box<dyn std::error::Error>> {
    println!("Looking for the mods folder");

    if !Path::new((dirs::home_dir().unwrap().display().to_string() + SteamFolder).as_str()).exists() {
        println!("mods folder not found");
    }

    let binding = fs::read_to_string(dirs::home_dir().unwrap().display().to_string() + SteamFolder).unwrap();
    let mut libs = Vdf::parse(binding.as_str())?;

    for key in libs.value.clone().unwrap_obj().keys() {
        for val in libs.value.clone().unwrap_obj().get(key) {
            for lib in val.iter() {
                for lib_key in lib.clone().unwrap_obj().keys() {
                    if lib_key == "apps" {
                        for libval in lib.clone().unwrap_obj().get(lib_key) {
                            for apps in libval.iter() {
                                for app in apps.clone().unwrap_obj().keys() {
                                    if app.as_str() == MegaMixAppID {
                                        println!("Found the library{:?}", lib.clone().unwrap_obj().get("path"));
                                        // this
                                        let mylib = lib.clone().unwrap_obj();
                                        // is
                                        let mypath = mylib.get("path").unwrap();
                                        // really
                                        let mystring = mypath.first().unwrap().to_string();
                                        // dumb
                                        let mut chars = mystring.chars();
                                        // all of that just so I can remove the fucking quotes on the library path
                                        chars.next();
                                        chars.next_back();
                                        diva_data.mods_directory = format!("{}{}", chars.as_str(), DivaModFolderSuffix).to_string();
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}


fn save_mod_configs(mut diva_data: &mut DivaData) {
    for diva_mod in &diva_data.mods {
        println!("{}\n{}", &diva_mod.path, toml::to_string(&diva_mod.config).unwrap());
    }
}
fn save_mod_config(mut diva_mod: &DivaMod) {
    println!("{}\n{}", &diva_mod.path, toml::to_string(&diva_mod.config).unwrap());
}