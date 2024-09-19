use std::cmp::{max, min};
use std::collections::HashMap;
use std::error::Error;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;
use std::{fs, io};

use compress_tools::{list_archive_files, uncompress_archive, Ownership};
use curl::easy::Easy;
use rfd::AsyncFileDialog;
use serde::{Deserialize, Serialize};
use slint::private_unstable_api::re_exports::ColorScheme;
use slint::{ComponentHandle, EventLoopError, Model, ModelRc, VecModel, Weak};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::config::{write_config, write_config_sync, write_dml_config};
use crate::diva::{get_diva_folder, get_temp_folder, open_error_window};
use crate::modpacks::{apply_mod_priority, ModPackMod};
use crate::slint_generatedApp::App;
use crate::{
    ConfirmDelete, DivaLogic, DivaModElement, EditModDialog, ModLogic, WindowLogic, DIVA_DIR,
};
use crate::{Download, DIVA_CFG, DML_CFG, MODS};

#[derive(Clone, Deserialize, Serialize)]
pub struct DivaModConfig {
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    #[serde(default)]
    pub dll: Vec<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub date: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
}

#[derive(Clone)]
pub struct DivaMod {
    pub config: DivaModConfig,
    pub path: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct DivaModLoader {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub console: bool,
    #[serde(default)]
    pub mods: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    ///
    /// This field tells dml what order to load mods in.
    ///
    /// It also happens that it will also only load mods in the array.
    ///
    /// The items are the name of the folder that the mod is stored in.
    ///
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub priority: Vec<String>,
}

impl DivaModLoader {
    pub(crate) fn new() -> Self {
        Self {
            enabled: false,
            console: false,
            mods: "mods".to_string(),
            version: "".to_string(),
            priority: vec![],
        }
    }
}

impl DivaMod {
    pub fn to_element(self: &Self) -> DivaModElement {
        let this = self.clone();
        DivaModElement {
            name: this.config.name.clone().into(),
            author: this.config.author.clone().into(),
            description: this.config.description.clone().into(),
            version: this.config.version.clone().into(),
            enabled: this.config.enabled,
            path: this.path.into(),
        }
    }
    pub fn to_packmod(self: &Self) -> ModPackMod {
        ModPackMod {
            name: self.config.name.clone(),
            enabled: true,
            path: self.path.clone(),
        }
    }

    pub fn dir_name(self: &Self) -> Option<String> {
        let mut buf = PathBuf::from(self.path.clone());
        buf.pop();
        if buf.exists() {
            return match buf.file_name() {
                Some(s) => Some(s.to_str().unwrap().to_string()),
                None => None,
            };
        }
        None
    }
}

impl DivaModElement {
    pub fn to_packmod(self: &Self) -> ModPackMod {
        ModPackMod {
            name: self.name.to_string(),
            enabled: self.enabled,
            path: self.path.to_string(),
        }
    }

    pub fn dir_name(self: &Self) -> Option<String> {
        let mut buf = PathBuf::from(self.path.to_string().clone());
        buf.pop();
        if buf.exists() {
            return match buf.file_name() {
                Some(s) => Some(s.to_str().unwrap().to_string()),
                None => None,
            };
        }
        None
    }
}

pub async fn init(
    ui: &App,
    dl_rx: Receiver<(i32, Download)>,
    dark_rx: tokio::sync::broadcast::Receiver<ColorScheme>,
) {
    let ui_toggle_handle = ui.as_weak();
    let ui_load_handle = ui.as_weak();
    let ui_progress_handle = ui.as_weak();
    let ui_download_handle = ui.as_weak();
    let ui_file_picker_handle = ui.as_weak();
    let ui_mod_up_handle = ui.as_weak();
    let ui_mod_down_handle = ui.as_weak();
    let ui_priority_handle = ui.as_weak();
    let ui_scheme_handle = ui.as_weak();
    let ui_edit_handle = ui.as_weak();

    let (dl_ui_tx, dl_ui_rx) = tokio::sync::mpsc::channel::<(i32, f32)>(2048);
    // setup thread for downloading, this will listen for Download objects sent on a tokio channel

    ui.on_load_mods(move || match load_mods() {
        Ok(_) => {
            let mods = get_mods_in_order();
            let _ = set_mods_table(&mods, ui_load_handle.clone());
        }
        Err(e) => {
            open_error_window(e.to_string());
            // eprintln!("{e}");
        }
    });

    let weak = ui.as_weak();
    ui.global::<DivaLogic>().on_download_dml(move || {
        let weak = weak.clone();
        tokio::spawn(async move {
            match get_latest_dml().await {
                Ok(release) => {
                    let release = release.clone();
                    let mut version_opt = None;
                    {
                        if let Ok(cfg) = DIVA_CFG.try_lock() {
                            version_opt = Some(cfg.dml_version.clone());
                        }
                    }
                    if version_opt.is_some_and(|v| v != release.name) {
                        if let Some(asset) = release.assets.first() {
                            println!("Downloading New DML Version");
                            match download_dml(asset.clone()).await {
                                Ok(source) => {
                                    if source.exists() {
                                        println!("Extracting DML");
                                        if let Some(diva_dir) = get_diva_folder() {
                                            let dest = PathBuf::from(diva_dir);
                                            match compress_tools::uncompress_archive(
                                                File::open(source).unwrap(),
                                                dest.as_path(),
                                                Ownership::Ignore,
                                            ) {
                                                Ok(_) => {
                                                    if let Ok(mut cfg) = DIVA_CFG.try_lock() {
                                                        cfg.dml_version = release.name.clone();
                                                        let cfg = cfg.clone();
                                                        if let Err(e) = write_config_sync(cfg) {
                                                            open_error_window(e.to_string());
                                                            return;
                                                        }
                                                    }
                                                    sleep(Duration::from_millis(50));
                                                    let _ = apply_mod_priority().await;
                                                    let _ = weak.upgrade_in_event_loop(move |ui| {
                                                        ui.set_dml_version(
                                                            release.name.clone().into(),
                                                        );
                                                    });
                                                }
                                                Err(e) => {
                                                    open_error_window(e.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    open_error_window(e.to_string());
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    open_error_window(e.to_string());
                }
            }
        });
    });

    ui.global::<ModLogic>().on_toggle_mod(move |module| {
        if let Ok(mut gmods) = MODS.lock() {
            if let Some(m) = gmods.get_mut(&module.dir_name().unwrap()) {
                m.config.enabled = !m.config.enabled;
                let buf = PathBuf::from(m.path.clone());
                println!("{}", buf.display());
                // fs::write(buf,
                if let Err(e) = save_mod_config(buf, &mut m.config.clone()) {
                    let msg = format!("Unable to save mod config: \n{}", e.to_string());
                    open_error_window(msg);
                }
            }
        }
        let mods = get_mods_in_order();
        let _ = set_mods_table(&mods, ui_toggle_handle.clone());
    });

    ui.global::<ModLogic>().on_move_mod_up(move |idx, amt| {
        let ui_mod_up_handle = ui_mod_up_handle.clone();
        if idx < 1 {
            return;
        }
        if let Ok(mut gcfg) = DIVA_CFG.lock() {
            if amt == 1 {
                gcfg.priority.swap(idx as usize, (idx - 1) as usize);
            } else {
                let m = gcfg.priority.remove(idx as usize);
                let len = gcfg.priority.len().clone();
                if amt == -1 {
                    gcfg.priority.insert(0, m);
                } else {
                    gcfg.priority
                        .insert((idx - amt).clamp(0, (len - 1) as i32) as usize, m);
                }
            }
            let lcfg = gcfg.clone();
            tokio::spawn(async move {
                match write_config(lcfg).await {
                    Ok(_) => {
                        // let gmods = MODS.lock().unwrap();
                        if let Err(e) =
                            set_mods_table(&get_mods_in_order(), ui_mod_up_handle.clone())
                        {
                            let msg =
                                format!("Unable to save priority to disk: \n{}", e.to_string());
                            open_error_window(msg);
                        }
                    }
                    Err(e) => {
                        let msg = format!("Unable to save priority to disk: \n{}", e.to_string());
                        open_error_window(msg);
                    }
                }
            });
        }
    });
    ui.global::<ModLogic>().on_move_mod_down(move |idx, amt| {
        let ui_mod_down_handle = ui_mod_down_handle.clone();
        if idx < 0 {
            return;
        }
        if let Ok(mut gcfg) = DIVA_CFG.lock() {
            if idx >= (gcfg.priority.len() - 1) as i32 {
                return;
            }
            if amt == 1 {
                gcfg.priority.swap(idx as usize, (idx + 1) as usize);
            } else {
                let m = gcfg.priority.remove(idx as usize);
                let len = gcfg.priority.len().clone();
                if amt == -1 {
                    gcfg.priority.push(m);
                } else {
                    gcfg.priority
                        .insert((idx + amt).clamp(0, (len - 1) as i32) as usize, m);
                }
            }
            let lcfg = gcfg.clone();
            tokio::spawn(async move {
                match write_config(lcfg).await {
                    Ok(_) => {
                        // let gmods = MODS.lock().unwrap();
                        if let Err(e) =
                            set_mods_table(&get_mods_in_order(), ui_mod_down_handle.clone())
                        {
                            let msg =
                                format!("Unable to save priority to disk: \n{}", e.to_string());
                            open_error_window(msg);
                        }
                    }
                    Err(e) => {
                        let msg = format!("Unable to save priority to disk: \n{}", e.to_string());
                        open_error_window(msg);
                    }
                }
            });
        }
    });

    ui.on_open_file_picker(move || {
        let picker = AsyncFileDialog::new()
            .add_filter("Archives", &["zip", "rar", "7z", "tar.gz"])
            .set_directory(dirs::home_dir().unwrap());
        let ui_file_picker_handle = ui_file_picker_handle.clone();
        tokio::spawn(async move {
            let res = picker.pick_file().await;
            if let Some(file_handle) = res {
                match unpack_mod_path(file_handle.path().to_path_buf()).await {
                    Ok(_) => {
                        // waiting for this because idk, sometimes something goes wrong and the table fails to load properly will need to debug later
                        tokio::time::sleep(Duration::from_millis(5)).await;
                        if load_mods().is_ok() {
                            let _ = set_mods_table(&get_mods_in_order(), ui_file_picker_handle);
                        }
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                        open_error_window(e.to_string());
                    }
                }
            }
        });
    });

    ui.global::<ModLogic>()
        .on_set_priority(move |_module, old, new| {
            if let Ok(mut cfg) = DIVA_CFG.lock() {
                let item = cfg.priority.remove(old as usize);
                let new = max(0, min(new as usize, cfg.priority.len()));
                cfg.priority.insert(new, item);
                let lcfg = cfg.clone();
                let ui_priority_handle = ui_priority_handle.clone();
                tokio::spawn(async move {
                    match write_config(lcfg).await {
                        Ok(_) => {
                            // let gmods = MODS.lock().unwrap();
                            if let Err(e) =
                                set_mods_table(&get_mods_in_order(), ui_priority_handle.clone())
                            {
                                let msg =
                                    format!("Unable to save priority to disk: \n{}", e.to_string());
                                open_error_window(msg);
                            }
                        }
                        Err(e) => {
                            let msg =
                                format!("Unable to save priority to disk: \n{}", e.to_string());
                            open_error_window(msg);
                        }
                    }
                });
            }
        });

    let scheme_rx = dark_rx.resubscribe();
    ui.global::<WindowLogic>()
        .on_open_mod_editor(move |module, _idx| {
            let ui_edit_handle = ui_edit_handle.clone();
            let editdialog = EditModDialog::new().unwrap();
            let current_scheme = ui_scheme_handle.upgrade().unwrap().get_color_scheme();
            editdialog.invoke_set_color_scheme(current_scheme);
            println!("{module:?}");
            editdialog.set_name(module.name);
            if let Ok(cfg_str) = fs::read_to_string(PathBuf::from(module.path.to_string())) {
                editdialog.set_module(cfg_str.into());
            }
            let weak = editdialog.as_weak();
            editdialog.on_cancel(move || {
                weak.upgrade().unwrap().hide().unwrap();
            });
            let weak = editdialog.as_weak();
            editdialog.on_apply(move || {
                let ui = weak.upgrade().unwrap();
                let m = ui.get_module().to_string();
                match toml::from_str::<DivaModConfig>(m.as_str()) {
                    Ok(mut cfg) => {
                        if let Err(e) =
                            save_mod_config(PathBuf::from(module.path.to_string()), &mut cfg)
                        {
                            open_error_window(e.to_string());
                        } else {
                            if let Ok(_) = load_mods() {
                                let _ =
                                    set_mods_table(&get_mods_in_order(), ui_edit_handle.clone());
                                ui.hide().unwrap();
                            }
                        }
                        // weak.upgrade()
                    }
                    Err(e) => open_error_window(e.to_string()),
                }
            });
            let weak = editdialog.as_weak();

            let mut scheme_rx = scheme_rx.resubscribe();
            let scheme_task = tokio::spawn(async move {
                while let Ok(scheme) = scheme_rx.recv().await {
                    let _ = weak.upgrade_in_event_loop(move |ui| {
                        ui.invoke_set_color_scheme(scheme);
                    });
                }
            });

            editdialog.window().on_close_requested(move || {
                scheme_task.abort();
                slint::CloseRequestResponse::HideWindow
            });

            editdialog.show().unwrap();
        });

    // ui.global::<ModLogic>().i
    let weak = ui.as_weak();
    ui.global::<ModLogic>().on_delete_mod(move |module| {
        let ui_weak = weak.clone();
        let ui = weak.upgrade().unwrap();
        let confirm = ConfirmDelete::new().unwrap();
        confirm.invoke_set_color_scheme(ui.get_color_scheme());
        confirm.set_item(module.name);
        let cweak = confirm.as_weak();
        confirm.on_close(move || {
            cweak.unwrap().hide().unwrap();
        });
        let cweak = confirm.as_weak();

        confirm.on_confirm(move || {
            cweak.unwrap().hide().unwrap();
            let mut buf = PathBuf::from(module.path.to_string());
            buf.pop();
            if let Ok(buf) = buf.canonicalize() {
                if buf.eq(&PathBuf::from("/").canonicalize().unwrap()) {
                    return;
                }
                match fs::remove_dir_all(buf) {
                    Ok(_) => {
                        if let Ok(_) = load_mods() {
                            if let Err(e) = set_mods_table(&get_mods_in_order(), ui_weak.clone()) {
                                open_error_window(e.to_string());
                            }
                        }
                    }
                    Err(e) => open_error_window(e.to_string()),
                }
            }
        });
        confirm.show().unwrap();
    });

    let _ = spawn_download_listener(dl_rx, dl_ui_tx, ui_download_handle);
    println!("dl spawned");
    let _ = spawn_download_ui_updater(dl_ui_rx, ui_progress_handle);
    println!("ui updater spawned");
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
                let msg = format!("Unable to create default mods folder: {}", e);
                eprintln!("{msg}");
                open_error_window(msg);
            }
        }
        return mods;
    }

    let paths = fs::read_dir(mods_folder).unwrap();
    for path in paths {
        let mut mod_path = path.unwrap().path().clone();
        if mod_path.is_file() || !mod_path.clone().is_dir() {
            println!(
                "Not a mod folder: {}",
                mod_path.clone().display().to_string()
            );
            continue;
        }

        // let mod_buf = PathBuf::from()
        mod_path.push("config.toml");
        let mod_p_str = mod_path.clone().display().to_string();
        match fs::read_to_string(mod_path.clone()) {
            Ok(s) => {
                let mod_config_res: Result<DivaModConfig, _> = toml::from_str(s.as_str());
                if mod_config_res.is_err() {
                    println!(
                        "Failed to read mod config for: {}",
                        mod_path.clone().display().to_string()
                    );
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
                println!(
                    "Not a mod folder: {}",
                    mod_path.clone().display().to_string()
                );
                continue;
            }
        }
    }
    mods
}

pub fn save_mod_config(
    config_path: PathBuf,
    diva_mod_config: &mut DivaModConfig,
) -> std::io::Result<()> {
    if let Ok(config_str) = toml::to_string(&diva_mod_config) {
        return match fs::write(config_path, config_str) {
            Ok(..) => {
                println!("Successfully updated config for {}", diva_mod_config.name);
                Ok(())
            }
            Err(e) => Err(e.into()),
        };
    }
    return Err(std::io::Error::new(ErrorKind::Other, "IDK"));
}

pub async fn unpack_mod_path(archive: PathBuf) -> compress_tools::Result<()> {
    let mut buf = PathBuf::from(get_diva_folder().unwrap_or("./mods".to_string()));
    // DIVA_CFG.lock().unwrap().
    buf.push(DML_CFG.lock().unwrap().mods.clone());
    // buf.push(diva.clone().dml.unwrap().mods);
    let name = archive
        .file_name()
        .unwrap_or(OsStr::new("missing.zip"))
        .to_str()
        .unwrap()
        .to_string();
    // let name = buf.extension().unwrap_or(OsStr::new("zip")).to_str().unwrap().to_string();
    let valid = check_archive_valid_structure(File::open(archive.clone()).unwrap(), name);
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

pub fn check_archive_valid_structure(archive: File, name: String) -> bool {
    println!("name: {}", name);
    let rar = name.ends_with(".rar");
    return match list_archive_files(archive) {
        Ok(files) => {
            let mut count = 0;
            for file in files {
                println!("{}", file);
                // zip spec uses / not \ so windows will be fine - WagYourTail, 2024
                if !file.contains("/") {
                    // this logic might work now
                    // rar archive work around since folders are not represented with a trailing /
                    if rar && count < 1 {
                        count += 1;
                    } else {
                        println!("aw dang it");
                        return false;
                    }
                }
            }
            // true if archive contains a single folder and nothing else at the root
            count <= 1
        }
        Err(e) => {
            eprintln!("{}", e);
            open_error_window(e.to_string());
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
    let res = toml::from_str::<DivaModLoader>(fs::read_to_string(buf).unwrap().as_str());
    let mut loader: Option<DivaModLoader> = None;
    match res {
        Ok(diva_ml) => {
            loader = Some(diva_ml);
        }
        Err(e) => {
            eprintln!("Failed to read data: {}", e)
        }
    }
    return loader;
}

pub fn spawn_download_listener(
    mut dl_rx: Receiver<(i32, Download)>,
    prog_tx: Sender<(i32, f32)>,
    ui_download_handle: Weak<App>,
) {
    // let diva_arc = diva_arc.clone();
    let ui_download_handle = ui_download_handle.clone();
    tokio::spawn(async move {
        println!("Listening for downloads");
        while !dl_rx.is_closed() {
            // Vec::is_empty()
            if let Some((index, download)) = dl_rx.recv().await {
                println!("{}", download.url.as_str());
                let mut dst = Vec::new();
                let mut easy = Easy::new();
                let mut rcvd = 0;
                let mut lstsnd = 0;
                easy.url(download.url.as_str()).unwrap();
                let _redirect = easy.follow_location(true);
                let mut started = false;

                {
                    let mut transfer = easy.transfer();
                    transfer
                        .write_function(|data| {
                            if !started {
                                started = true;
                                println!("First chunk received");
                            }
                            dst.extend_from_slice(data);
                            rcvd += data.len();
                            let prog = rcvd - lstsnd;
                            if lstsnd as i32 == 0 || prog >= 3000 {
                                lstsnd = rcvd.clone();
                                let p = prog_tx.try_send((index.clone(), prog as f32));
                                match p {
                                    Ok(_) => {}
                                    Err(_e) => {
                                        // eprintln!("{}", e);
                                    }
                                }
                            }
                            Ok(data.len())
                        })
                        .unwrap();

                    // handle the error here instead of unwrapping so that this
                    // receiver thread doesn't panic and downloads can continue to happen
                    match transfer.perform() {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("{}", e);
                            open_error_window(e.to_string());
                            let _ = ui_download_handle.upgrade_in_event_loop(move |ui| {
                                let downloads = ui.get_downloads_list();
                                if let Some(downloads) =
                                    downloads.as_any().downcast_ref::<VecModel<Download>>()
                                {
                                    if let Some(mut download) = downloads.row_data(index as usize) {
                                        download.failed = true;
                                        downloads.set_row_data(index as usize, download);
                                    }
                                }
                            });
                            // skip the file, wait for next download
                            continue;
                        }
                    }
                }
                let mut dl_path = PathBuf::from(get_temp_folder().unwrap());
                dl_path.push(&download.name.as_str());
                let file_res = File::create(dl_path.clone());
                match file_res {
                    Ok(mut f) => match f.write_all(dst.clone().as_slice()) {
                        Ok(_) => {
                            println!("Saved successfully, will try to extract");
                            match unpack_mod_path(dl_path.clone()).await {
                                Ok(_) => {
                                    println!("Successfully unpacked mod");
                                    if load_mods().is_ok() {
                                        let _ = set_mods_table(
                                            &get_mods_in_order(),
                                            ui_download_handle.clone(),
                                        );
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to extract the mod file:\n{}", e);
                                    // let _ = ui_download_handle.clone().upgrade_in_event_loop(
                                    //     move |ui| {
                                    let msg = format!(
                                        "Failed to extract the mod file: \n{}",
                                        e.to_string()
                                    );
                                    // ui.invoke_open_error_dialog(msg.into());
                                    // },
                                    // );
                                    open_error_window(msg);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Something went wrong while saving the file to disk \n{}", e);
                            let msg = format!(
                                "Something went wrong while saving the file to disk: \n{}",
                                e.to_string()
                            );
                            open_error_window(msg);
                        }
                    },
                    Err(e) => {
                        eprintln!("Something went wrong while saving the file to disk \n{}", e);
                        let msg = format!(
                            "Something went wrong while saving the file to disk: \n{}",
                            e.to_string()
                        );
                        open_error_window(msg);
                    }
                }
            }
        }
        println!("Closed idk");
    });
}

pub fn spawn_download_ui_updater(mut prog_rx: Receiver<(i32, f32)>, ui_weak: Weak<App>) {
    tokio::spawn(async move {
        let wait_time = tokio::time::Duration::from_millis(50);
        while !prog_rx.is_closed() {
            /*
            await recv causes thread to hang until download is finished
            so we are try_recv'ing instead
            but we can't do this without a timeout so it's be moved to wait
            a little bit before trying again
            Try_recv without the timeout causes preformance issues on the host's PC

            Time wasted trying to make await work = 3 hours
            */
            if let Ok((index, chunk_size)) = prog_rx.try_recv() {
                match ui_weak.upgrade_in_event_loop(move |ui| {
                    let downloads = ui.get_downloads_list();
                    if let Some(downloads) = downloads.as_any().downcast_ref::<VecModel<Download>>()
                    {
                        if let Some(mut download) = downloads.row_data(index as usize) {
                            download.progress += chunk_size as i32;
                            downloads.set_row_data(index as usize, download);
                        }
                    }
                }) {
                    Err(e) => {
                        eprintln!("{}", e);
                    }
                    _ => {}
                };
            } else {
                sleep(wait_time);
            }
        }
        println!("Progress listener Closed");
    });
}

pub fn set_mods_table(mods: &Vec<DivaMod>, ui_handle: Weak<App>) -> Result<(), EventLoopError> {
    let mods = mods.clone();
    ui_handle.upgrade_in_event_loop(move |ui| {
        let mods_model: VecModel<DivaModElement> = VecModel::default();
        for diva_mod in mods.clone() {
            mods_model.push(diva_mod.to_element());
        }
        let model = ModelRc::new(mods_model);
        ui.set_mods(model);
    })
}

pub fn load_mods() -> std::io::Result<()> {
    let dir = DIVA_DIR.lock().unwrap().to_string();
    let mut buf = PathBuf::from(dir);
    let mut gconf = DIVA_CFG.lock().unwrap();
    buf.push("mods");
    let buf = buf.canonicalize()?;
    buf.display().to_string();
    let mods = load_mods_from_dir(buf.display().to_string());
    let mut dmods = MODS.lock().unwrap();
    let mut mod_map = HashMap::new();
    for mut m in mods {
        let n = m.dir_name().unwrap_or(m.config.name.clone());

        // Will make the mod's name default to the folder name if the field is blank for some reason
        if m.config.name.is_empty() {
            m.config.name = n.clone();
        }

        mod_map.insert(n.clone(), m.clone());
        if !gconf.priority.contains(&n) {
            gconf.priority.push(n);
        }
    }
    *dmods = mod_map.clone();
    if mod_map.len() != gconf.priority.len() {
        let mut mods: Vec<DivaMod> = vec![];
        for p in gconf.priority.clone() {
            match dmods.get(&p) {
                Some(m) => {
                    mods.push(m.clone());
                }
                None => {}
            }
        }
        let mut prio = vec![];
        for m in mods {
            prio.push(m.dir_name().unwrap_or(m.config.name.clone()));
        }
        gconf.priority = prio.clone();
    }
    // clone and drop the mutex instance from here so it can be unlocked
    let gconf = gconf.clone();
    if gconf.applied_pack.is_empty() {
        if let Ok(mut dml) = DML_CFG.try_lock() {
            dml.priority = gconf.priority.clone();
            match write_dml_config(dml.clone()) {
                Ok(_) => {}
                Err(e) => eprintln!("{e}"),
            }
        }
    }
    Ok(())
}

pub fn get_mods_in_order() -> Vec<DivaMod> {
    let mut mods = vec![];
    let prio = DIVA_CFG.lock().unwrap().priority.clone();
    let gmods = MODS.lock().unwrap().clone();
    for p in prio {
        match gmods.get(&p) {
            Some(m) => {
                mods.push(m.clone());
            }
            None => {}
        }
    }
    mods
}

pub fn is_dml_installed() -> bool {
    return match get_diva_folder() {
        Some(dir) => {
            let mut buf = PathBuf::from(dir);
            buf.push("dinput8.dll");
            buf.exists()
        }
        None => false,
    };
}

static DML_LATEST_RELEASE: &'static str =
    "https://api.github.com/repos/blueskythlikesclouds/DivaModLoader/releases/latest";

#[derive(Clone, Deserialize, Serialize)]
pub struct GhRelease {
    name: String,
    assets: Vec<GhReleaseAsset>,
}

unsafe impl Send for GhRelease {}

#[derive(Clone, Deserialize, Serialize)]
pub struct GhReleaseAsset {
    name: String,
    browser_download_url: String,
}

pub async fn download_dml(asset: GhReleaseAsset) -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    if let Some(temp) = get_temp_folder() {
        let mut buf = PathBuf::from(temp);
        buf.push(asset.name.clone());
        let bytes = reqwest::get(asset.browser_download_url.clone())
            .await?
            .bytes()
            .await?;
        match tokio::fs::write(buf.clone(), bytes).await {
            Ok(_) => Ok(buf),
            Err(e) => Err(e.into()),
        }
    } else {
        Err(Box::new(io::Error::new(
            ErrorKind::NotFound,
            "Unable to get temp folder",
        )))
    }
    // Ok(PathBuf::new())
}

pub async fn get_latest_dml() -> Result<GhRelease, Box<dyn Error + Send + Sync>> {
    let builder = reqwest::ClientBuilder::new().user_agent(concat!(
        env!("CARGO_PKG_NAME"),
        ":",
        env!("CARGO_PKG_VERSION")
    ));
    let client = builder.build()?;
    let text = client.get(DML_LATEST_RELEASE).send().await?.text().await?;
    Ok(sonic_rs::from_str::<GhRelease>(&text)?)
}
