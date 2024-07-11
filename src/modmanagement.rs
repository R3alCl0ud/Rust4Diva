use std::fs;
use std::fs::File;
use std::path::Path;
use compress_tools::{Ownership, uncompress_archive};
use egui::TextBuffer;
use keyvalues_parser::Vdf;
use toml::de::Error;
use crate::{DIVA_MOD_FOLDER_SUFFIX, DivaData, DivaMod, DivaModConfig, DivaModLoader, MEGA_MIX_APP_ID, STEAM_FOLDER};

pub fn load_mods(diva_data: &mut DivaData) -> Vec<DivaMod> {
    let mods_folder = format!("{}/{}", diva_data.diva_directory.as_str().to_owned(),
                              diva_data.diva_mod_loader.mods.as_str());
    println!("Loading mods from {}", mods_folder);
    let mut mods: Vec<DivaMod> = Vec::new();

    if !Path::new(mods_folder.as_str()).exists() {
        println!("unable to load mods from nonexistent mods folder");
        return mods;
    }

    let paths = fs::read_dir(mods_folder).unwrap();
    for path in paths {
        let mod_path = path.unwrap().path().clone();
        if mod_path.is_file() || !mod_path.clone().is_dir() {
            continue;
        }
        let mut mod_config: DivaModConfig = toml::from_str(
            fs::read_to_string(mod_path.clone().display().to_string() + "/config.toml")
                .unwrap().as_str()).unwrap();
        // println!("Mod: {}, {}", mod_config.clone().name, mod_config.description.escape_default().to_string());
        mod_config.description = mod_config.description.escape_default().to_string();
        mods.push(DivaMod {
            config: mod_config,
            path: (mod_path.clone().display().to_string() + "/config.toml").to_string(),
        });
    }
    mods
}


pub fn get_diva_folder() -> Result<String, Box<dyn std::error::Error>> {
    println!("Looking for the mods folder");

    if !Path::new((dirs::home_dir().unwrap().display().to_string() + STEAM_FOLDER).as_str()).exists() {
        println!("mods folder not found");
    }
    let mut path = "".to_owned();
    let binding = fs::read_to_string(dirs::home_dir().unwrap().display().to_string() + STEAM_FOLDER).unwrap();
    let mut librariesfolders = Vdf::parse(binding.as_str())?;
    let mut libraries = librariesfolders.value.unwrap_obj();
    for library_id in libraries.clone().keys() {
        // get the library obj
        let mut library = libraries.get(library_id).unwrap().first().unwrap().clone().unwrap_obj();

        // prevent a crash in case of malformed libraryfolders.vdf
        if !library.contains_key("apps") || !library.contains_key("path") { continue; }

        // get the list of apps installed to this library
        let apps = library.get("apps").unwrap().first().unwrap().clone().unwrap_obj();


        // self-explanatory
        if apps.contains_key(MEGA_MIX_APP_ID) {
            // get the path of the library
            let path_str = library.get("path").unwrap().first().unwrap().to_string();
            // this is set up for removing the quotes
            let mut path_chars = path_str.chars();
            // remove the quotes from the value
            path_chars.next();
            path_chars.next_back();
            // concat the strings together properly
            path = format!("{}{}", path_chars.as_str(), DIVA_MOD_FOLDER_SUFFIX).to_string();
            println!("Fuck yes, we found it, {:?}", path);
            // diva_data.diva_mod_loader = toml::from_str((path + "/config.toml").as_str()).unwrap();
            // diva_data.mods_directory = path;
            break;
        }
    }
    Ok(path)
}


pub fn save_mod_configs(mut diva_data: &mut DivaData) {
    for diva_mod in &diva_data.mods {
        println!("{}\n{}", &diva_mod.path, toml::to_string(&diva_mod.config).unwrap());
    }
}
pub fn save_mod_config(path: &str, diva_mod_config: &mut DivaModConfig) {
    println!("{}\n{}", path, toml::to_string(&diva_mod_config).unwrap());
}

pub fn unpack_mod(mut mod_archive: File, diva_data: DivaData) {
    uncompress_archive(mod_archive, Path::new(format!("{}/{}", &diva_data.diva_directory, &diva_data.diva_mod_loader.mods).as_str()), Ownership::Preserve)
        .expect("Welp, wtf, idk what happened, must be out of space or someshit");
}

pub fn load_diva_ml_config(diva_folder: String) -> Option<DivaModLoader> {
    println!("{}{}", diva_folder, "/config.toml");
    let res: Result<DivaModLoader, Error> = toml::from_str(fs::read_to_string((diva_folder + "/config.toml")).unwrap().as_str());
    let mut loader: Option<DivaModLoader> = None;
    match res {
        Ok(diva_ml) => {
            loader = Some(diva_ml);
        }
        Err(e) => {
            panic!("Failed to write data: {}", e)
        }
    }
    return loader;
}