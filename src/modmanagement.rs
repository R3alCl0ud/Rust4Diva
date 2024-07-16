use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::mpsc;
use std::thread::JoinHandle;

use compress_tools::{Ownership, uncompress_archive};
use egui::{DroppedFile, TextBuffer};
use keyvalues_parser::Vdf;
use sonic_rs::{Deserialize, Serialize};
use toml::de::Error;

use crate::gamebanana_async::{GBMod, GbModDownload};

const STEAM_FOLDER: &str = "/.local/share/Steam/config/libraryfolders.vdf";
const MEGA_MIX_APP_ID: &str = "1761390";
const DIVA_MOD_FOLDER_SUFFIX: &str = "/steamapps/common/Hatsune Miku Project DIVA Mega Mix Plus";


// begin structs for the diva configs and stuff

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
}

///
/// Represents a diva mod after it has been loaded in to program memory
///
///
#[derive(Clone)]
struct DivaMod {
    config: DivaModConfig,
    path: String,
}

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
    dml: DivaModLoader,
    dlthreads: Vec<(JoinHandle<()>, mpsc::SyncSender<egui::Context>)>,
    dl_done_tx: mpsc::SyncSender<f32>,
    dl_done_rc: mpsc::Receiver<f32>,
    should_dl: bool,
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

/// Parses the libraryfolders.vdf in the user's home folder it exists
///
/// If we are unable to fix the install location of Project Diva then an empty string is returned
pub fn get_diva_folder() -> Result<String, Box<dyn std::error::Error>> {
    println!("Looking for the mods folder");

    if !Path::new((dirs::home_dir().unwrap().display().to_string() + STEAM_FOLDER).as_str()).exists() {
        println!("Steam install directory not found");
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
    let config_path = Path::new(path);
    if let config_str = toml::to_string(&diva_mod_config).unwrap() {
        match fs::write(config_path, config_str) {
            Ok(..) => {
                println!("Sucessfully updated config for {}", diva_mod_config.name);
            }
            Err(e) => {
                eprintln!("Something went wrong: {:#?}", e);
            }
        }
    }
}

pub fn old_unpack_mod(mod_archive: File, diva_data: &&mut DivaData) {
    uncompress_archive(mod_archive, Path::new(format!("{}/{}", &diva_data.diva_directory, &diva_data.dml.mods).as_str()), Ownership::Preserve)
        .expect("Welp, wtf, idk what happened, must be out of space or some shit");
}

pub fn unpack_mod(module: &GbModDownload, diva_data: &&mut DivaData) {
    let module_path = "/tmp/rust4diva/".to_owned() + &*module._sFile;
    if let file = File::open(&module_path).unwrap() {
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