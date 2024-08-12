use std::collections::HashMap;
use std::env;
use std::sync::{Arc, LazyLock};

use slint_interpreter::ComponentHandle;
use tokio::sync::Mutex;

use crate::config::{load_diva_config, write_config, DivaConfig};
use crate::diva::{create_tmp_if_not, get_diva_folder, MIKU_ART};
use crate::gamebanana_async::{parse_dmm_url, GBSearch, GbModDownload};
use crate::modmanagement::{
    load_diva_ml_config, load_mods, set_mods_table, DivaMod, DivaModLoader, load_mods_too,
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
struct DivaData {
    mods: Vec<DivaMod>,
    search_results: Vec<GBSearch>,
    mods_directory: String,
    diva_directory: String,
    dml: Option<DivaModLoader>,
    mod_files: HashMap<u64, Vec<GbModDownload>>,
    mod_packs: HashMap<String, ModPack>,
    config: DivaConfig,
}

pub static MODS_VEC: std::sync::Mutex<Vec<DivaMod>> = std::sync::Mutex::new(Vec::new());
pub static DIVA_DIR: LazyLock<std::sync::Mutex<String>> =
    LazyLock::new(|| std::sync::Mutex::new(String::new()));
pub static MODS_DIR: LazyLock<std::sync::Mutex<String>> =
    LazyLock::new(|| std::sync::Mutex::new(String::new()));
// pub static DML_CFG: std::sync::Mutex<DivaModLoader> = std::sync::Mutex::new(DivaModLoader { enabled: false, console: false, mods: "".to_string(), version: "".to_string(), priority: vec![] });

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
        Err(_) => {}
    }

    let mut diva_state = DivaData::new();
    let diva_dir = get_diva_folder();

    if diva_dir.is_some() {
        diva_state.diva_directory = diva_dir.unwrap();
        let mut dir = DIVA_DIR.lock().expect("fuck");
        *dir = diva_state.diva_directory.clone();
    }

    if let Ok(cfg) = load_diva_config().await {
        diva_state.config = cfg;
    }

    diva_state.dml = load_diva_ml_config(&diva_state.diva_directory.as_str());

    if diva_state.dml.is_none() {
        diva_state.dml = Some(DivaModLoader::new());
    }

    diva_state.mods = load_mods(&diva_state);

    let _ =  load_mods_too();



    for m in &diva_state.mods {
        let ps: Vec<&str> = m.path.split("/").collect();
        if ps.len() > 2 {
            let dir = ps[ps.len() - 2];
            diva_state.config.priority.push(dir.to_string());
        }
    }
    match write_config(diva_state.config.clone()).await {
        Ok(_) => {}
        Err(_) => {}
    }

    set_mods_table(&diva_state.mods, app_weak.clone());

    if let Some(dml) = &diva_state.dml {
        app.set_dml_version(dml.version.clone().into());
        app.set_dml_enabled(dml.enabled);
    }

    let diva_arc = Arc::new(Mutex::new(diva_state));
    modmanagement::init(&app, Arc::clone(&diva_arc), dl_rx).await;
    gamebanana_async::init(&app, Arc::clone(&diva_arc), dl_tx, url_rx).await;
    modpacks::init(&app, Arc::clone(&diva_arc)).await;

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
            config: DivaConfig { priority: vec![], diva_dir: "".to_string() },
        }
    }
}
