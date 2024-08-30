#![windows_subsystem = "windows"]


use std::collections::HashMap;
use std::env;
use std::sync::{Arc, LazyLock};

use modmanagement::get_mods_in_order;
use slint::{SharedString, VecModel};
use slint_interpreter::ComponentHandle;
use tokio::sync::Mutex;

use crate::config::{load_diva_config, write_config, DivaConfig};
use crate::diva::{create_tmp_if_not, get_diva_folder, MIKU_ART};
use crate::gamebanana_async::{parse_dmm_url, GBSearch, GbModDownload};
use crate::modmanagement::{
    load_diva_ml_config, load_mods, set_mods_table, DivaMod, DivaModLoader,
};
use crate::modpacks::ModPack;
use crate::oneclick::{spawn_listener, try_send_mmdl};

mod config;
mod diva;
mod gamebanana_async;
mod modmanagement;
mod modpacks;
mod oneclick;

slint::include_modules!();

#[derive(Clone)]
pub struct DivaData {
    mods: Vec<DivaMod>,
    search_results: Vec<GBSearch>,
    mods_directory: String,
    diva_directory: String,
    dml: Option<DivaModLoader>,
    mod_files: HashMap<u64, Vec<GbModDownload>>,
    mod_packs: HashMap<String, ModPack>,
    config: DivaConfig,
}

pub static MODS: LazyLock<std::sync::Mutex<HashMap<String, DivaMod>>> =
    LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));
pub static DIVA_DIR: LazyLock<std::sync::Mutex<String>> = LazyLock::new(|| {
    let mut str = String::new();
    if let Some(dir_str) = get_diva_folder() {
        str = dir_str;
    }
    return std::sync::Mutex::new(str);
});
pub static MODS_DIR: LazyLock<std::sync::Mutex<String>> =
    LazyLock::new(|| std::sync::Mutex::new("mods".to_string()));

/// Global config object
pub static DIVA_CFG: LazyLock<std::sync::Mutex<DivaConfig>> =
    LazyLock::new(|| std::sync::Mutex::new(DivaConfig::new()));

/// Global HashMap of ModPacks key'd by modpack name
pub static MOD_PACKS: LazyLock<std::sync::Mutex<HashMap<String, ModPack>>> =
    LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

pub static DML_CFG: LazyLock<std::sync::Mutex<DivaModLoader>> = LazyLock::new(|| {
    let mut cfg = None;
    if let Ok(dir) = DIVA_DIR.lock() {
        cfg = load_diva_ml_config(dir.as_str());
    }
    std::sync::Mutex::new(cfg.unwrap_or(DivaModLoader::new()))
});

#[tokio::main]
async fn main() {
    println!("Starting Rust4Diva Slint Edition");
    println!("{}", MIKU_ART);
    let args = env::args();

    let mut dmm_url = None;

    for arg in args {
        match parse_dmm_url(arg.clone()) {
            Some(_dmm) => {
                println!("{}", arg.clone());
                dmm_url = Some(arg.clone());
                match try_send_mmdl(arg.clone()).await {
                    Ok(_) => {
                        return;
                    }
                    Err(e) => {
                        eprintln!("Unable to send to existing rust4diva instance, will handle here instead\n{}", e);
                    }
                }
                break;
            }
            None => {}
        }
    }
    create_tmp_if_not().expect("Failed to create temp directory, now we are panicking");

    let app = App::new().unwrap();
    let app_weak = app.as_weak();

    let (url_tx, url_rx) = tokio::sync::mpsc::channel(2048);
    let (dl_tx, dl_rx) = tokio::sync::mpsc::channel::<(i32, Download)>(2048);

    // rx.try_recv();
    match spawn_listener(url_tx.clone(), app_weak.clone()).await {
        Ok(_) => {}
        Err(e) => {
            let msg = format!(
                "Unable start listener: \n{}",
                e.to_string()
            );
            app_weak.clone().upgrade().unwrap().invoke_open_error_dialog(msg.into());
        }
    }

    let mut diva_state = DivaData::new();

    if let Ok(cfg) = load_diva_config().await {
        diva_state.config = cfg.clone();
        let mut gcfg = DIVA_CFG.lock().expect("msg");
        *gcfg = cfg.clone();
    }

    if let Some(diva_dir) = get_diva_folder() {
        diva_state.diva_directory = diva_dir;
        let mut dir = DIVA_DIR.lock().expect("fuck");
        *dir = diva_state.diva_directory.clone();
    }

    diva_state.dml = load_diva_ml_config(&diva_state.diva_directory.as_str());

    if diva_state.dml.is_none() {
        diva_state.dml = Some(DivaModLoader::new());
    }

    let _ = load_mods();
    let _ = set_mods_table(&get_mods_in_order(), app_weak.clone());

    if let Some(dml) = &diva_state.dml {
        app.set_dml_version(dml.version.clone().into());
        app.set_dml_enabled(dml.enabled);
    }

    let diva_arc = Arc::new(Mutex::new(diva_state));
    modmanagement::init(&app, Arc::clone(&diva_arc), dl_rx).await;
    gamebanana_async::init(&app, Arc::clone(&diva_arc), dl_tx, url_rx).await;
    modpacks::init(&app, Arc::clone(&diva_arc)).await;
    config::init_ui(&app).await;

    println!("Does the app run?");

    if let Some(url) = dmm_url {
        println!("We have a url to handle");
        match url_tx.clone().send(url).await {
            Ok(_) => {}
            Err(e) => {
                eprintln!("{}", e);
            }
        }
    }

    app.run().expect("Welp, gui thread paniced");
    println!("OMG Migu says \"goodbye\"");
    // Ok(())
}

impl DivaData {
    fn new() -> Self {
        Self {
            mods: Vec::new(),
            search_results: Vec::new(),
            mods_directory: "".to_string(),
            diva_directory: "".to_string(),
            dml: None,
            mod_files: HashMap::new(),
            mod_packs: Default::default(),
            config: DivaConfig::new(),
        }
    }
}
