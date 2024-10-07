#![windows_subsystem = "windows"]

use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::sync::{LazyLock, Mutex};

use config::write_config;
use diva::get_rust4diva_version;
use modmanagement::{is_dml_installed, is_dml_installed_at};
use slint::private_unstable_api::re_exports::ColorScheme;
use slint_interpreter::ComponentHandle;
use tokio::sync::broadcast;

use crate::config::{load_diva_config, DivaConfig};
#[cfg(not(debug_assertions))]
use crate::diva::MIKU_ART;
use crate::diva::{create_tmp_if_not, find_diva_folder, open_error_window};
use crate::gamebanana::parse_dmm_url;
use crate::modmanagement::{
    get_mods, load_diva_ml_config, load_mods, set_mods_table, DivaMod, DivaModLoader,
};
use crate::modpacks::ModPack;
use crate::oneclick::{spawn_listener, try_send_mmdl};

mod config;
mod diva;
mod firstlaunch;
mod gamebanana;
mod language;
mod modmanagement;
mod modpacks;
mod oneclick;
mod util;

slint::include_modules!();

pub static MODS: LazyLock<Mutex<HashMap<String, DivaMod>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
pub static DIVA_DIR: LazyLock<Mutex<String>> = LazyLock::new(|| {
    let str = String::new();
    Mutex::new(str)
});
pub static MODS_DIR: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new("mods".to_string()));

/// Global config object
pub static R4D_CFG: LazyLock<Mutex<DivaConfig>> = LazyLock::new(|| Mutex::new(DivaConfig::new()));

/// Global HashMap of ModPacks key'd by modpack name
pub static MOD_PACKS: LazyLock<Mutex<HashMap<String, ModPack>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub static DML_CFG: LazyLock<Mutex<DivaModLoader>> = LazyLock::new(|| {
    let mut cfg = None;
    if let Ok(dir) = DIVA_DIR.lock() {
        cfg = load_diva_ml_config(dir.as_str());
    }
    Mutex::new(cfg.unwrap_or(DivaModLoader::new()))
});

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn Error>> {
    println!("Starting Rust4Diva Slint Edition");
    #[cfg(not(debug_assertions))]
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

    let (url_tx, url_rx) = tokio::sync::mpsc::channel(2048);

    let mut r4d_config = match load_diva_config().await {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("{e}");
            DivaConfig::new()
        }
    };

    {
        let mut gcfg = R4D_CFG
            .lock()
            .expect("Config should not have panic already");
        if !is_dml_installed_at(&r4d_config.diva_dir) {
            println!("DML Not installed");
            r4d_config.dml_version = "".to_owned();
            let _ = write_config(r4d_config.clone()).await;
        }
        *gcfg = r4d_config.clone();
    }

    if !r4d_config.use_system_scaling {
        #[cfg(debug_assertions)]
        println!("Trying to set scale factor: {}", r4d_config.scale);
        env::set_var("SLINT_SCALE_FACTOR", r4d_config.scale.to_string());
    }

    if let Ok(scale) = env::var("SLINT_SCALE_FACTOR") {
        println!("Got scale from env: {scale}");
    }

    env::set_var("SLINT_BACKEND", "winit");

    let app = App::new()?;
    language::init_ui(&app).await;

    if r4d_config.use_system_theme {
        app.invoke_set_color_scheme(ColorScheme::Unknown);
    } else if r4d_config.dark_mode {
        app.invoke_set_color_scheme(ColorScheme::Dark);
    } else {
        app.invoke_set_color_scheme(ColorScheme::Light);
    }

    if let Some(diva_dir) = find_diva_folder() {
        let mut dir = DIVA_DIR.lock()?;
        *dir = diva_dir;
    }

    if !is_dml_installed() {
        app.invoke_ask_install_dml();
    } else {
        app.set_dml_version(r4d_config.dml_version.clone().into());
    }
    app.set_b_dirname(r4d_config.use_dirname);

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
    let _ = load_mods();
    let _ = set_mods_table(&get_mods(), app_weak.clone());
    if is_dml_installed() {
        if let Ok(dml) = DML_CFG.try_lock() {
            app.set_dml_enabled(dml.enabled);
        }
    } else {
        app.set_dml_enabled(false);
    }

    app.set_r4d_version(get_rust4diva_version().into());

    config::init_ui(&app, dark_tx).await;
    modmanagement::init(&app, dark_rx.resubscribe()).await;
    modpacks::init(&app).await;
    gamebanana::init(&app, url_rx, dark_rx.resubscribe()).await;

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
    #[cfg(debug_assertions)]
    println!("Current Window Scale: {}", app.window().scale_factor());
    let _ = firstlaunch::init(&app).await;
    slint::run_event_loop()?;
    println!("OMG Migu says \"goodbye\"");
    Ok(())
}
