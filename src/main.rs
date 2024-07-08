mod gamebanana;

use std::{fs, path};
use crate::gamebanana::GbModDownload;
use eframe::{egui, Frame};
use egui::Context;
use serde::Deserialize;

const ModsFolder: &str = "/home/neptune/.local/share/Steam/steamapps/common/Hatsune Miku Project DIVA Mega Mix Plus/mods";

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
    loads: i32
}


fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };
    println!("Hello, world!");
    eframe::run_native(
        "My egui App",
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
            loads: 0
        }
    }
}

impl eframe::App for DivaData {
    fn update(&mut self, ctx: &Context, frame: &mut Frame) {
        if self.loads < 1 {
            load_mods(self);
            self.loads += 1;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Rust4Diva Mod Manager");
            for divamod in &self.mods {
                ui.horizontal(|iui| {
                    iui.label(divamod.clone().name);
                    iui.label(divamod.clone().enabled.to_string());
                    if iui.button("Toggle").clicked() {
                        // this doesn't actually toggle the mod yet
                        println!("{} has been toggled", divamod.clone().name);
                    }
                });
            }
        });
    }
}

fn load_mods(mut diva_data: &mut DivaData) {
    let paths = fs::read_dir(ModsFolder).unwrap();

    for path in paths {
        let mod_path = path.unwrap().path().clone();
        if mod_path.clone().is_file() || !mod_path.clone().is_dir() {
            continue;
        }
        let mod_config:DivaMod = toml::from_str(fs::read_to_string(mod_path.clone().display().to_string() +"/config.toml").unwrap().as_str()).unwrap();
        println!("Mod: {}", mod_config.clone().name);
        diva_data.mods.push(mod_config);
    }
}