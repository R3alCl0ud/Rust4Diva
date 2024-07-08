mod gamebanana;

use std::{fs, path};
use std::collections::HashMap;
use std::path::Path;
use crate::gamebanana::GbModDownload;
use eframe::{egui, Frame};
use egui::{Context, TextBuffer};
use keyvalues_parser::Vdf;
use serde::Deserialize;

const ModsFolder: &str = "~/.local/share/Steam/steamapps/common/Hatsune Miku Project DIVA Mega Mix Plus/mods";
const SteamFolder: &str = "/.local/share/Steam/config/libraryfolders.vdf";
const MegaMixAppID: &str = "1761390";
const DivaModFolderSuffix: &str = "/steamapps/common/Hatsune Miku Project DIVA Mega Mix Plus/mods";

#[derive(Clone, Deserialize)]
struct DivaMod {
    enabled: bool,
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
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
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
        let mut mods = Vec::new();


        Self {
            mods,
            downloads: Vec::new(),
            loaded: false,
            mods_directory: ModsFolder.parse().unwrap(),
        }
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
            ui.heading("Rust4Diva Mod Manager");
            egui::ScrollArea::vertical().show(ui, |sui| {
                egui::Grid::new("mod_grid").show(sui, |gui| {
                    for divamod in &self.mods {
                        gui.label(divamod.clone().name);
                        gui.label(divamod.clone().enabled.to_string());
                        if gui.button("Toggle").clicked() {
                            // this doesn't actually toggle the mod yet
                            println!("{} has been toggled", divamod.clone().name);
                        }
                        gui.end_row();
                    }
                });
            });
            if ui.button("Reload").clicked() {
                load_mods(self);
                self.loaded = true;
            }
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
        let mod_config: DivaMod = toml::from_str(fs::read_to_string(mod_path.clone().display().to_string() + "/config.toml").unwrap().as_str()).unwrap();
        println!("Mod: {}", mod_config.clone().name);
        mods.push(mod_config);
    }
    mods
}


fn get_diva_mods_folder(mut diva_data: &mut DivaData) -> Result<(), Box<dyn std::error::Error>> {
    println!("Looking for the mods folder");

    if !Path::new((dirs::home_dir().unwrap().display().to_string() + SteamFolder).as_str()).exists() {
        println!("mods folder not found");
    }

    let binding = fs::read_to_string(dirs::home_dir().unwrap().display().to_string() + SteamFolder).unwrap();
    let libvdf = binding.as_str();
    // println!("{}", libvdf.clone());

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