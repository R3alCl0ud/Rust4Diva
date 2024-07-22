// use eframe::App;
// use crate::App;
use std::{env, fs};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use compress_tools::{Ownership, uncompress_archive};
use keyvalues_parser::Vdf;
use serde::{Deserialize, Serialize};
use slint::{ComponentHandle, Model, ModelRc, StandardListViewItem, VecModel, Weak};
use tokio::sync::Mutex;
use toml::de::Error;

use crate::DivaData;
use crate::gamebanana_async::GbModDownload;
use crate::slint_generatedApp::App;


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

pub fn load_mods(diva_data: &DivaData) -> Vec<DivaMod> {
    load_mods_from_dir(format!("{}/{}", diva_data.diva_directory.as_str().to_owned(),
                               diva_data.dml.mods.as_str()))
}

pub fn load_mods_from_dir(dir: String) -> Vec<DivaMod> {
    let mods_folder = dir;
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
        "windows" => {
        }
        _ => { println!("Unsupported Operating system: {}", env::consts::OS) }
    }

    steam_str
}


pub fn get_diva_folder() -> Option<String> {
    println!("Looking for the mods folder");
    match get_steam_folder() {
        Some(steam_folder) => {
            let mut path = "".to_owned();
            let mut lib_path = PathBuf::new();
            lib_path.push(steam_folder);
            lib_path.push(STEAM_LIBRARIES_CONFIG);
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
    let module_path = "/tmp/rust4diva/".to_owned() + &*module.file;
    if let Ok(file) = File::open(&module_path) {
        unpack_mod(file, diva_data);
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
            panic!("Failed to read data: {}", e)
        }
    }
    return loader;
}

pub fn create_tmp_if_not() -> std::io::Result<()> {
    let path = Path::new("/tmp/rust4diva");
    if !path.exists() {
        let dir = fs::create_dir(path);
        return dir;
    }
    Ok(())
}


pub fn one_click() {
    println!("Test");
}

pub async fn init(ui: &App, diva_arc: Arc<Mutex<DivaData>>) {
    let ui_toggle_handle = ui.as_weak();
    let ui_load_handle = ui.as_weak();
    let toggle_diva = diva_arc.clone();
    let load_diva = diva_arc.clone();
    let diva_state = diva_arc.lock().await;
    let mods_dir = format!("{}/{}", &diva_state.diva_directory.as_str().to_owned(),
                           &diva_state.dml.mods.as_str());

    ui.on_load_mods(move || {
        let app = ui_load_handle.upgrade().unwrap();
        app.set_counter(app.get_counter() + 1);
        let mods = load_mods_from_dir(mods_dir.clone());
        let model_vec: VecModel<ModelRc<StandardListViewItem>> = VecModel::default();
        for item in &mods {
            let items: Rc<VecModel<StandardListViewItem>> = Rc::new(VecModel::default());
            let enable_str = if item.config.enabled { "Enabled" } else { "Disabled" };
            let enabled = StandardListViewItem::from(enable_str);
            let name = StandardListViewItem::from(item.config.name.as_str());
            let authors = StandardListViewItem::from(item.config.author.as_str());
            let version = StandardListViewItem::from(item.config.version.as_str());
            let description = StandardListViewItem::from(item.config.description.as_str());
            items.push(enabled);
            items.push(name);
            items.push(authors);
            items.push(version);
            items.push(description);
            model_vec.push(items.into());
        }
        let model = ModelRc::new(model_vec);
        app.set_stuff(model);
        let darc = load_diva.clone();
        tokio::spawn(async move {
            darc.lock().await.mods = mods.clone();
        });
    });


    ui.on_toggle_mod(move |row_num| {
        if row_num > -1 {
            let row_num: usize = row_num as usize;
            // let value = ui_toggle_handle.clone();
            let toggle_diva = Arc::clone(&toggle_diva);
            let ui_toggle_handle = ui_toggle_handle.clone();
            tokio::spawn(async move {
                let mut darc = &mut toggle_diva.lock().await;
                if darc.mods.len() < row_num {
                    return;
                }
                let mods_dir = darc.mods_directory.clone();
                let mut module = &mut darc.mods[row_num];
                let mod_path = format!("{}{}", mods_dir, &module.path.clone());
                module.config.enabled = !module.config.enabled;
                let mut module = module.clone();
                ui_toggle_handle.upgrade_in_event_loop(move |ui| {
                    let stuff_bind = ui.get_stuff();
                    if let Some(mut model_vec) = stuff_bind.as_any().downcast_ref::<VecModel<ModelRc<StandardListViewItem>>>() {
                        if let Some(item) = model_vec.row_data(row_num) {
                            let enable_str = if module.config.enabled { "Enabled" } else { "Disabled" };
                            let enabled = StandardListViewItem::from(enable_str);
                            item.set_row_data(0, enabled);
                            save_mod_config(mod_path.as_str(), &mut module.config);
                        }
                    }
                }).unwrap();
            });
        }
    });
}

pub fn push_mods_to_table(mods: &Vec<DivaMod>, weak: Weak<App>) {
    let app = weak.upgrade().unwrap();
    let binding = app.get_stuff();
    if let Some(model_vec) = binding.as_any().downcast_ref::<VecModel<ModelRc<StandardListViewItem>>>() {
        for item in mods {
            let items: Rc<VecModel<StandardListViewItem>> = Rc::new(VecModel::default());
            let enable_str = if item.config.enabled { "Enabled" } else { "Disabled" };
            let enabled = StandardListViewItem::from(enable_str);
            let name = StandardListViewItem::from(item.config.name.as_str());
            let authors = StandardListViewItem::from(item.config.author.as_str());
            let version = StandardListViewItem::from(item.config.version.as_str());
            let description = StandardListViewItem::from(item.config.description.as_str());
            items.push(enabled);
            items.push(name);
            items.push(authors);
            items.push(version);
            items.push(description);
            model_vec.push(items.into());
        }
    }
}
