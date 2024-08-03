use crate::DivaModElement;
use std::{env, fs};
use std::fs::File;
use std::io::{ErrorKind, Seek, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use compress_tools::{list_archive_files, Ownership, uncompress_archive};
use curl::easy::Easy;
use keyvalues_parser::Vdf;
use rfd::AsyncFileDialog;
use serde::{Deserialize, Serialize};
use slint::{ComponentHandle, Model, ModelRc, StandardListViewItem, VecModel, Weak};
use sonic_rs::JsonValueTrait;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use toml::de::Error;

use crate::{DivaData, Download};
use crate::slint_generatedApp::App;

cfg_if::cfg_if! {
    if #[cfg(windows)] {
        use winreg::enums::*;
        use winreg::RegKey;
    }
}

const STEAM_FOLDER: &str = ".local/share/Steam";
const STEAM_FOLDER_MAC: &str = "Library/Application Support/Steam";
const STEAM_LIBRARIES_CONFIG: &str = "config/libraryfolders.vdf";
const MEGA_MIX_APP_ID: &str = "1761390";
const DIVA_MOD_FOLDER_SUFFIX: &str = "steamapps/common/Hatsune Miku Project DIVA Mega Mix Plus";

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

impl DivaModLoader {
    pub(crate) fn new() -> Self {
        Self {
            enabled: false,
            console: false,
            mods: "mods".to_string(),
            version: "".to_string(),
        }
    }
}

impl DivaMod {
    pub fn to_element(self: &Self) -> DivaModElement {
        let this = self.clone();
        DivaModElement {
            name: this.config.name.clone().into(),
            author: this.config.name.clone().into(),
            description: this.config.name.clone().into(),
            version: this.config.version.clone().into(),
            enabled: this.config.enabled,
        }
    }
}

pub async fn init(ui: &App, diva_arc: Arc<Mutex<DivaData>>, dl_rx: Receiver<(i32, Download)>) {
    let ui_toggle_handle = ui.as_weak();
    let ui_load_handle = ui.as_weak();
    let ui_progress_handle = ui.as_weak();
    let ui_download_handle = ui.as_weak();
    let ui_file_picker_handle = ui.as_weak();
    let toggle_diva = diva_arc.clone();
    let load_diva = diva_arc.clone();
    let picker_diva = diva_arc.clone();
    let diva_state = diva_arc.lock().await;
    let mut mod_buf = PathBuf::from(&diva_state.diva_directory);
    mod_buf.push(diva_state.clone().dml.unwrap().mods.as_str());
    let mods_dir = mod_buf.display().to_string();

    let reload_dir = mods_dir.clone();
    let (dl_ui_tx, dl_ui_rx) = tokio::sync::mpsc::channel::<(i32, f32)>(2048);
    // setup thread for downloading, this will listen for Download objects sent on a tokio channel


    ui.on_load_mods(move || {
        let mods = load_mods_from_dir(mods_dir.clone());
        set_mods_table(&mods, ui_load_handle.clone());
        let darc = load_diva.clone();
        tokio::spawn(async move {
            darc.lock().await.mods = mods.clone();
        });
    });


    ui.on_toggle_mod(move |row_num| {
        if row_num > -1 {
            let row_num: usize = row_num as usize;
            let toggle_diva = Arc::clone(&toggle_diva);
            let ui_toggle_handle = ui_toggle_handle.clone();
            tokio::spawn(async move {
                let darc = &mut toggle_diva.lock().await;
                if darc.mods.len() < row_num {
                    return;
                }
                let mods_dir = darc.mods_directory.clone();
                let module = &mut darc.mods[row_num];
                let mut buf = PathBuf::from(mods_dir.clone());
                buf.push(&module.path.clone());
                let mod_path = buf.display().to_string();
                module.config.enabled = !module.config.enabled;
                let mut module = module.clone();
                match save_mod_config(mod_path.as_str(), &mut module.config) {
                    Ok(_) => {
                        set_mods_table(&darc.mods.clone(), ui_toggle_handle);
                    }
                    Err(_e) => {}
                }
            });
        }
    });
    ui.on_open_file_picker(move || {
        let picker = AsyncFileDialog::new()
            .add_filter("Archives", &["zip", "rar", "7z", "tar.gz"])
            .set_directory(dirs::home_dir().unwrap());
        let picker_diva = picker_diva.clone();
        let unpack_diva = picker_diva.clone();
        let ui_file_picker_handle = ui_file_picker_handle.clone();
        tokio::spawn(async move {
            let res = picker.pick_file().await;
            if let Some(file_handle) = res {
                match unpack_mod_path(file_handle.path().to_path_buf(), unpack_diva).await {
                    Ok(_) => {
                        let darc = picker_diva.clone();
                        let mut diva = darc.lock().await;
                        let mods = load_mods_from_dir(diva.mods_directory.clone());
                        set_mods_table(&mods, ui_file_picker_handle);
                        diva.mods = mods;
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                    }
                }
            }
        });
    });


    let _ = spawn_download_listener(dl_rx, dl_ui_tx, &diva_arc.clone(), reload_dir, ui_download_handle);
    let _ = spawn_download_ui_updater(dl_ui_rx, ui_progress_handle);
}


pub fn load_mods(diva_data: &DivaData) -> Vec<DivaMod> {
    if diva_data.dml.is_none() {
        return vec![];
    }
    let mut path_buf = PathBuf::new();
    path_buf.push(diva_data.diva_directory.as_str());
    path_buf.push(diva_data.clone().dml.unwrap().mods.as_str());
    let os_str = path_buf.as_os_str().to_str();
    if os_str.is_none() {
        return vec![];
    }
    load_mods_from_dir(os_str.unwrap().to_string())
}

pub fn load_mods_from_dir(dir: String) -> Vec<DivaMod> {
    let mods_folder = dir;
    println!("Loading mods from {}", mods_folder);
    let mut mods: Vec<DivaMod> = Vec::new();

    if mods_folder == "" {
        return mods;
    }

    if !Path::new(mods_folder.as_str()).exists() {
        println!("unable to load mods from nonexistent mods folder, creating default folder");
        match fs::create_dir(mods_folder) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Unable to create default mods folder: {}", e);
            }
        }
        return mods;
    }

    let paths = fs::read_dir(mods_folder).unwrap();
    for path in paths {
        let mut mod_path = path.unwrap().path().clone();
        if mod_path.is_file() || !mod_path.clone().is_dir() {
            println!("Not a mod folder: {}", mod_path.clone().display().to_string());
            continue;
        }

        // let mod_buf = PathBuf::from()
        mod_path.push("config.toml");
        let mod_p_str = mod_path.clone().display().to_string();
        match fs::read_to_string(mod_path.clone()) {
            Ok(s) => {
                let mod_config_res: Result<DivaModConfig, _> = toml::from_str(s.as_str());
                if mod_config_res.is_err() {
                    println!("Failed to read mod config for: {}", mod_path.clone().display().to_string());
                    continue;
                }
                let mut mod_config = mod_config_res.unwrap();
                mod_config.description = mod_config.description.escape_default().to_string();
                mods.push(DivaMod {
                    config: mod_config,
                    path: mod_p_str,
                });
            }
            Err(_) => {
                println!("Not a mod folder: {}", mod_path.clone().display().to_string());
                continue;
            }
        }
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
                println!("Regular Steam folder not found, searching for flatpak isntead");
                binding = dirs::home_dir().unwrap();
                binding.push(".var/app/com.valvesoftware.Steam/data/Steam");
                if !binding.exists() {
                    println!("Can't find flatpak steam.");
                    return None;
                }
            }
            steam_str = Some(binding.display().to_string());
        }
        "macos" => {
            let mut binding = dirs::home_dir().unwrap();
            binding.push(STEAM_FOLDER_MAC);
            if !binding.exists() {
                println!("Steam folder not found");
            }
            steam_str = Some(binding.display().to_string());
        }
        "windows" => {
            // only compiles on windows
            cfg_if::cfg_if! {
                if #[cfg(windows)] {
                    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
                    let steam_key = hklm.open_subkey(r#"SOFTWARE\WOW6432Node\Valve\Steam"#);
                    if steam_key.is_err() {
                        return None;
                    }
                    let steam_key = steam_key.unwrap();
                    let res: std::io::Result<String> = steam_key.get_value("InstallPath");
                    if let Ok(path) = res {
                        println!("{}", path);
                        steam_str = Some(path);
                    }
                }
            }
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
            if !lib_path.exists() {
                return None;
            }

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
                            let mut buf = PathBuf::new();
                            buf.push(path_chars.as_str());
                            let diva = PathBuf::from(DIVA_MOD_FOLDER_SUFFIX);
                            buf.push(diva.as_os_str());
                            path = buf.canonicalize().unwrap().as_os_str().to_str().unwrap().to_string();
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

pub fn save_mod_config(path: &str, diva_mod_config: &mut DivaModConfig) -> std::io::Result<()> {
    let config_path = Path::new(path);
    if let Ok(config_str) = toml::to_string(&diva_mod_config) {
        return match fs::write(config_path, config_str) {
            Ok(..) => {
                println!("Successfully updated config for {}", diva_mod_config.name);
                Ok(())
            }
            Err(e) => {
                Err(e.into())
            }
        };
    }
    return Err(std::io::Error::new(ErrorKind::Other, "IDK"));
}

pub async fn unpack_mod(mut mod_archive: File, diva_arc: Arc<Mutex<DivaData>>) -> compress_tools::Result<()> {
    let diva = diva_arc.lock().await;
    let mut buf = PathBuf::from(&diva.diva_directory);
    buf.push(diva.clone().dml.unwrap().mods);
    // let path = buf.display().to_string();
    mod_archive.rewind()?;
    uncompress_archive(mod_archive, buf.as_path(), Ownership::Ignore)
}

pub async fn unpack_mod_path(archive: PathBuf, diva_arc: Arc<Mutex<DivaData>>) -> compress_tools::Result<()> {
    let diva = diva_arc.lock().await;
    let mut buf = PathBuf::from(&diva.diva_directory);
    buf.push(diva.clone().dml.unwrap().mods);
    let valid = check_archive_valid_structure(File::open(archive.clone()).unwrap());
    println!("Good structure? {}", valid);
    if !valid {
        buf.push(archive.file_name().unwrap());
        if !buf.exists() {
            let _ = fs::create_dir(buf.clone());
        }
    }
    let mod_archive = File::open(archive.clone()).unwrap();
    uncompress_archive(mod_archive, buf.as_path(), Ownership::Ignore)
}


pub fn check_archive_valid_structure(archive: File) -> bool {
    return match list_archive_files(archive) {
        Ok(files) => {
            for file in files {
                println!("{}", file);
                // zip spec uses / not \ so windows will be fine - WagYourTail, 2024
                if !file.contains("/") {
                    return false;
                }
            }
            true
        }
        Err(e) => {
            eprintln!("{}", e);
            false
        }
    };
}


pub fn load_diva_ml_config(diva_folder: &str) -> Option<DivaModLoader> {
    let mut buf = PathBuf::from(diva_folder);
    buf.push("config.toml");
    if !buf.exists() {
        return None;
    }
    let res: Result<DivaModLoader, Error> = toml::from_str(fs::read_to_string(buf).unwrap().as_str());
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
    match get_temp_folder() {
        Some(p) => {
            let path = Path::new(&p);
            if !path.exists() {
                let dir = fs::create_dir(path);
                return dir;
            }
            Ok(())
        }
        None => {
            Err(std::io::Error::new(ErrorKind::InvalidInput, "Unknown temp dir path"))
        }
    }
}


pub fn spawn_download_listener(mut dl_rx: Receiver<(i32, Download)>, prog_tx: Sender<(i32, f32)>, diva_arc: &Arc<Mutex<DivaData>>, mods_dir: String, ui_download_handle: Weak<App>) {
    let diva_arc = diva_arc.clone();
    let ui_download_handle = ui_download_handle.clone();
    tokio::spawn(async move {
        println!("Listening for downloads");
        while !dl_rx.is_closed() {
            if let Some((index, download)) = dl_rx.recv().await {
                println!("{}", download.url.as_str());
                let mut dst = Vec::new();
                let mut easy = Easy::new();
                easy.url(download.url.as_str()).unwrap();
                let _redirect = easy.follow_location(true);
                let mut started = false;

                {
                    let mut transfer = easy.transfer();
                    transfer.write_function(|data| {
                        if !started {
                            started = true;
                            println!("First chunk received");
                        }
                        dst.extend_from_slice(data);
                        let p = prog_tx.try_send((index.clone(), data.len() as f32));
                        match p {
                            Ok(_) => {}
                            Err(e) => {
                                eprintln!("{}", e);
                            }
                        }
                        Ok(data.len())
                    }).unwrap();

                    // handle the error here instead of unwrapping so that this
                    // receiver thread doesn't panic and downloads can continue to happen
                    match transfer.perform() {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("{}", e);
                            let _ = ui_download_handle.upgrade_in_event_loop(move |ui| {
                                let downloads = ui.get_downloads_list();
                                if let Some(downloads) = downloads.as_any().downcast_ref::<VecModel<Download>>() {
                                    if let Some(mut download) = downloads.row_data(index as usize) {
                                        download.failed = true;
                                        downloads.set_row_data(index as usize, download);
                                    }
                                }
                            });
                            // skip the file shit, wait for next download
                            continue;
                        }
                    }
                }
                let mut dl_path = PathBuf::from(get_temp_folder().unwrap());
                dl_path.push(&download.name.as_str());
                let file_res = File::create(dl_path.clone());
                match file_res {
                    Ok(mut f) => {
                        match f.write_all(dst.clone().as_slice()) {
                            Ok(_) => {
                                println!("Saved successfully, will try to extract");
                                match unpack_mod_path(dl_path.clone(), diva_arc.clone()).await {
                                    Ok(_) => {
                                        println!("Successfully unpacked mod");
                                        let mods = load_mods_from_dir(mods_dir.clone());
                                        let mut diva = diva_arc.lock().await;
                                        diva.mods = mods.clone();
                                        ui_download_handle.upgrade_in_event_loop(move |ui| {
                                            let model = create_mods_model(&mods.clone());
                                            ui.set_stuff(model);
                                        }).expect("failed to update the mods list after unpacking mod");
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to extract the mod file:\n{}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Something went wrong while saving the file to disk \n{}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Something went wrong while saving the file to disk \n{}", e);
                    }
                }
            }
        }
        println!("Closed idk");
    });
}


pub fn spawn_download_ui_updater(mut prog_rx: Receiver<(i32, f32)>, ui_weak: Weak<App>) {
    tokio::spawn(async move {
        while !prog_rx.is_closed() {
            if let Ok((index, chunk_size)) = prog_rx.try_recv() {
                match ui_weak.upgrade_in_event_loop(move |ui| {
                    let downloads = ui.get_downloads_list();
                    if let Some(downloads) = downloads.as_any().downcast_ref::<VecModel<Download>>() {
                        if let Some(mut download) = downloads.row_data(index as usize) {
                            download.progress += chunk_size;
                            downloads.set_row_data(index as usize, download);
                        }
                    }
                }) {
                    Err(e) => {
                        eprintln!("{}", e);
                    }
                    _ => {}
                };
            }
        }
        println!("Progress listener Closed");
    });
}


pub fn create_mods_model(mods: &Vec<DivaMod>) -> ModelRc<ModelRc<StandardListViewItem>> {
    let model_vec: VecModel<ModelRc<StandardListViewItem>> = VecModel::default();
    for item in mods.iter() {
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
    ModelRc::new(model_vec)
}


pub fn get_temp_folder() -> Option<String> {
    match env::consts::OS {
        "linux" | "macos" => {
            Some("/tmp/rust4diva".to_string())
        }
        "windows" => {
            let mut tmp = dirs::data_local_dir().unwrap();
            tmp.push("Temp");
            let temp = tmp.as_os_str();
            match temp.to_str() {
                Some(s) => {
                    let t = s.to_owned();
                    Some(t)
                }
                None => {
                    None
                }
            }
        }
        os => {
            println!("Unknown OS: {}", os);
            None
        }
    }
}


pub fn set_mods_table(mods: &Vec<DivaMod>, ui_handle: Weak<App>) {
    let mods = mods.clone();
    ui_handle.upgrade_in_event_loop(move |ui| {
        let vecmod: VecModel<DivaModElement> = VecModel::default();
        for module in mods.clone() {
            vecmod.push(module.to_element());
        }
        let model = ModelRc::new(vecmod);
        ui.set_mods(model);
    }).unwrap();
}
