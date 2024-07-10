use std::fs;
use std::path::Path;
use keyvalues_parser::Vdf;
use crate::{DivaData, DivaMod, DivaModConfig, DivaModFolderSuffix, MegaMixAppID, SteamFolder};

pub fn load_mods(diva_data: &mut DivaData) -> Vec<DivaMod> {
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
        println!("Mod: {}, {}", mod_config.clone().name, mod_config.description.escape_default().to_string());
        mod_config.description = mod_config.description.escape_default().to_string();
        mods.push(DivaMod {
            config: mod_config,
            path: (mod_path.clone().display().to_string() + "/config.toml").to_string(),
        });
    }
    mods
}


pub fn get_diva_mods_folder(diva_data: &mut DivaData) -> Result<(), Box<dyn std::error::Error>> {
    println!("Looking for the mods folder");

    if !Path::new((dirs::home_dir().unwrap().display().to_string() + SteamFolder).as_str()).exists() {
        println!("mods folder not found");
    }

    let binding = fs::read_to_string(dirs::home_dir().unwrap().display().to_string() + SteamFolder).unwrap();
    let mut librariesfolders = Vdf::parse(binding.as_str())?;
    let mut libraries = librariesfolders.value.unwrap_obj();
    for library_id in libraries.clone().keys() {
        // get the library obj
        let mut library = libraries.get(library_id).unwrap().first().unwrap().clone().unwrap_obj();
        // get the list of apps installed to this library
        let apps = library.get("apps").unwrap().first().unwrap().clone().unwrap_obj();
        // get the path of the library
        let path_str = library.get("path").unwrap().first().unwrap().to_string();
        let mut path_chars = path_str.chars();
        // remove the quotes from the value
        path_chars.next();
        path_chars.next_back();
        // concat the strings together properly
        let path = format!("{}{}", path_chars.as_str(), DivaModFolderSuffix).to_string();
        if apps.contains_key(MegaMixAppID) {
            println!("Fuck yes, we found it, {:?}", path);
            diva_data.mods_directory = path;
            break;
        }
    }
    Ok(())
}
