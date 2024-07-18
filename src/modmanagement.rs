use std::{env, fs};
use std::fs::File;
use std::path::{Path, PathBuf};

use compress_tools::{Ownership, uncompress_archive};
use keyvalues_parser::Vdf;
use serde::{Deserialize, Serialize};
use toml::de::Error;

use crate::DivaData;
use crate::gamebanana_async::GbModDownload;

const STEAM_FOLDER: &str = ".local/share/Steam";
const STEAM_LIBRARIES_CONFIG: &str = "config/libraryfolders.vdf";
const MEGA_MIX_APP_ID: &str = "1761390";
const DIVA_MOD_FOLDER_SUFFIX: &str = "/steamapps/common/Hatsune Miku Project DIVA Mega Mix Plus";

#[derive(Clone, Deserialize, Serialize)]
pub struct DivaModConfig {
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) include: Vec<String>,
    #[serde(default)]
    pub(crate) dll: Vec<String>,
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) version: String,
    #[serde(default)]
    pub(crate) date: String,
    #[serde(default)]
    pub(crate) author: String,
}

#[derive(Clone)]
pub struct DivaMod {
    pub(crate) config: DivaModConfig,
    pub(crate) path: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct DivaModLoader {
    #[serde(default)]
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) console: bool,
    #[serde(default)]
    pub(crate) mods: String,
    #[serde(default)]
    pub(crate) version: String,
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

pub fn load_mods(diva_data: &mut DivaData) -> Vec<DivaMod> {
    let mods_folder = format!("{}/{}", diva_data.diva_directory.as_str().to_owned(),
                              diva_data.dml.mods.as_str());
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
        let mod_config_res: Result<DivaModConfig, _> = toml::from_str(
            fs::read_to_string(mod_path.clone().display().to_string() + "/config.toml")
                .unwrap().as_str());
        if mod_config_res.is_err() {
            continue;
        }
        let mut mod_config = mod_config_res.unwrap();
        // println!("Mod: {}, {}", mod_config.clone().name, mod_config.description.escape_default().to_string());
        mod_config.description = mod_config.description.escape_default().to_string();
        mods.push(DivaMod {
            config: mod_config,
            path: (mod_path.clone().display().to_string() + "/config.toml").to_string(),
        });
    }
    mods
}


pub fn get_steam_folder() -> Option<String> {
    let mut steam_str = None;
    println!("Attempting to find the Steam folder");
    match env::consts::OS {
        "linux" => {
            let mut binding = dirs::home_dir().unwrap();
            binding.push(STEAM_FOLDER);
            if !binding.exists() {
                println!("Steam folder not found");
            }
            steam_str = Some(binding.display().to_string());
        }
        _ => { println!("Unsupported Operating system: {}", env::consts::OS) }
    }

    steam_str
}


pub fn get_diva_folder() -> Option<String> {
    println!("Looking for the mods folder");
    match get_steam_folder() {
        Some(steam_folder) => {
            println!("{}", steam_folder);
            let mut path = "".to_owned();
            let mut lib_path = PathBuf::new();
            lib_path.push(steam_folder);
            lib_path.push(STEAM_LIBRARIES_CONFIG);
            println!("{}", lib_path.display());
            let binding = fs::read_to_string(lib_path).unwrap();
            let lf_res = Vdf::parse(binding.as_str());
            match lf_res {
                Ok(libraryfolders) => {
                    let libraries = libraryfolders.value.unwrap_obj();
                    for library_id in libraries.clone().keys() {
                        // get the library obj
                        let library = libraries.get(library_id).unwrap().first().unwrap().clone().unwrap_obj();
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
                            break;
                        }
                    }
                    Some(path)
                }
                Err(e) => {
                    eprintln!("{}", e);
                    None
                }
            }
        }
        None => {
            return None;
        }
    }
}


pub fn save_mod_configs(diva_data: &mut DivaData) {
    for diva_mod in &diva_data.mods {
        println!("{}\n{}", &diva_mod.path, toml::to_string(&diva_mod.config).unwrap());
    }
}
pub fn save_mod_config(path: &str, diva_mod_config: &mut DivaModConfig) {
    println!("{}\n{}", path, toml::to_string(&diva_mod_config).unwrap());
    let config_path = Path::new(path);
    if let Ok(config_str) = toml::to_string(&diva_mod_config) {
        match fs::write(config_path, config_str) {
            Ok(..) => {
                println!("Successfully updated config for {}", diva_mod_config.name);
            }
            Err(e) => {
                eprintln!("Something went wrong: {:#?}", e);
            }
        }
    }
}

pub fn unpack_mod(mod_archive: File, diva_data: &&mut DivaData) {
    uncompress_archive(mod_archive, Path::new(format!("{}/{}", &diva_data.diva_directory, &diva_data.dml.mods).as_str()), Ownership::Preserve)
        .expect("Welp, wtf, idk what happened, must be out of space or some shit");
}

pub fn unpack_mod_from_temp(module: &GbModDownload, diva_data: &&mut DivaData) {
    let module_path = "/tmp/rust4diva/".to_owned() + &*module._sFile;
    if let Ok(file) = File::open(&module_path) {
        uncompress_archive(file, Path::new(format!("{}/{}", &diva_data.diva_directory, &diva_data.dml.mods).as_str()), Ownership::Preserve)
            .expect("Welp, wtf, idk what happened, must be out of space or some shit");
    } else {
        eprintln!("Unable to open archive at {:#}", module_path);
    }
}

pub fn load_diva_ml_config(diva_folder: &str) -> Option<DivaModLoader> {
    println!("{}{}", diva_folder, "/config.toml");
    let res: Result<DivaModLoader, Error> = toml::from_str(fs::read_to_string(diva_folder.to_string() + "/config.toml").unwrap().as_str());
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

pub fn create_tmp_if_not() -> std::io::Result<()> {
    let path = Path::new("/tmp/rust4diva");
    if !path.exists() {
        let dir = fs::create_dir(path);
        // return dir
        return dir;
    }
    Ok(())
}


pub fn one_click() {
    println!("Test");
}