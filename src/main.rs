#![windows_subsystem = "windows"]

use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::sync::{Arc, LazyLock};

use modmanagement::{get_mods_in_order, is_dml_installed};
use slint::private_unstable_api::re_exports::ColorScheme;
use slint_interpreter::ComponentHandle;
use tokio::sync::{broadcast, Mutex};

use crate::config::{load_diva_config, DivaConfig};
use crate::diva::{create_tmp_if_not, get_diva_folder, open_error_window, MIKU_ART};
use crate::gamebanana_async::parse_dmm_url;
use crate::modmanagement::{
    load_diva_ml_config, load_mods, set_mods_table, DivaMod, DivaModLoader,
};
use crate::modpacks::ModPack;
use crate::oneclick::{spawn_listener, try_send_mmdl};

use slint::Weak;

mod config;
mod diva;
mod firstlaunch;
mod gamebanana_async;
mod modmanagement;
mod modpacks;
mod oneclick;

slint::include_modules!();

#[derive(Clone)]
pub struct DivaData {
    mods: Vec<DivaMod>,
    diva_directory: String,
    dml: Option<DivaModLoader>,
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

pub static MAIN_UI_WEAK: std::sync::Mutex<Option<Weak<App>>> = std::sync::Mutex::new(None);

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn Error>> {
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
                        return Ok(());
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

    let (dark_tx, dark_rx) = broadcast::channel::<ColorScheme>(200);

    let app = App::new()?;
    let app_weak = app.as_weak();

    if let Ok(mut weak_opt) = MAIN_UI_WEAK.try_lock() {
        *weak_opt = Some(app_weak.clone());
    }

    let (url_tx, url_rx) = tokio::sync::mpsc::channel(2048);
    let (dl_tx, dl_rx) = tokio::sync::mpsc::channel::<(i32, Download)>(2048);
    app.window().on_close_requested(move || {
        std::process::exit(0);
    });
    let app_weak = app.as_weak();

    match spawn_listener(url_tx.clone(), app_weak.clone()).await {
        Ok(_) => {}
        Err(e) => {
            let msg = format!("Unable start listener: \n{}", e.to_string());
            open_error_window(msg);
        }
    }

    if let Ok(cfg) = load_diva_config().await {
        let mut gcfg = DIVA_CFG
            .lock()
            .expect("Config should not have panic already");
        *gcfg = cfg.clone();
        if gcfg.dark_mode {
            app.invoke_set_color_scheme(ColorScheme::Dark);
        }
        if !gcfg.dark_mode {
            app.invoke_set_color_scheme(ColorScheme::Light);
        }
        app.set_dml_version(cfg.dml_version.clone().into());
    }

    if let Some(diva_dir) = get_diva_folder() {
        let mut dir = DIVA_DIR.lock()?;
        *dir = diva_dir;
        if !is_dml_installed() {
            app.invoke_ask_install_dml();
        }
    }

    let _ = load_mods();
    let _ = set_mods_table(&get_mods_in_order(), app_weak.clone());

    if let Ok(dml) = DML_CFG.try_lock() {
        app.set_dml_enabled(dml.enabled);
    }

    app.set_r4d_version(env!("CARGO_PKG_VERSION").into());

    modmanagement::init(&app, dl_rx, dark_rx.resubscribe()).await;
    gamebanana_async::init(&app, dl_tx, url_rx, dark_rx.resubscribe()).await;
    modpacks::init(&app).await;
    config::init_ui(&app, dark_tx).await;

    println!("Does the app run?");

    if let Some(url) = dmm_url {
        println!("We have a url to handle");
        match url_tx.clone().send(url).await {
            Ok(_) => {}
            Err(e) => {
                open_error_window(e.to_string());
            }
        }
    }

    app.show().expect("Window should have opened");
    let _ = firstlaunch::init(&app).await;
    slint::run_event_loop()?;
    println!("OMG Migu says \"goodbye\"");
    Ok(())
}

impl DivaData {
    fn new() -> Self {
        Self {
            mods: Vec::new(),
            diva_directory: "".to_string(),
            dml: None,
            config: DivaConfig::new(),
        }
    }
}
