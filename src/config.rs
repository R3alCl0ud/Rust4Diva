use std::error::Error;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Mutex;

use rfd::AsyncFileDialog;
use serde::{Deserialize, Serialize};
use slint::private_unstable_api::re_exports::ColorScheme;
use slint::{CloseRequestResponse, ComponentHandle};
use tokio::fs;
use tokio::sync::broadcast::Sender;

use crate::diva::{get_config_dir_sync, get_diva_folder, get_steam_folder, open_error_window};
use crate::modmanagement::DivaModLoader;
use crate::slint_generatedApp::App;
use crate::DML_CFG;

use crate::{
    diva::get_config_dir, DivaLogic, SettingsLogic, SettingsWindow, WindowLogic, DIVA_CFG,
};

#[derive(Deserialize, Serialize, Clone)]
pub struct DivaConfig {
    /// this is the global priority order, this is used when no modpack is applied
    pub priority: Vec<String>,
    pub diva_dir: String,
    #[serde(default)]
    pub steam_dir: String,
    #[serde(default)]
    pub aft_dir: String,
    #[serde(default)]
    pub applied_pack: String, // pub
    #[serde(default)]
    pub aft_mode: bool,
    #[serde(default = "yes")]
    pub dark_mode: bool,
    #[serde(default = "yes")]
    pub first_run: bool,
    #[serde(default)]
    pub dml_version: String,
}

fn yes() -> bool {
    true
}

impl DivaConfig {
    pub fn new() -> Self {
        Self {
            priority: vec![],
            diva_dir: "".to_string(),
            steam_dir: "".to_string(),
            aft_dir: "".to_string(),
            applied_pack: "".to_string(),
            aft_mode: false,
            dark_mode: true,
            first_run: true,
            dml_version: "".to_owned(),
        }
    }
}

pub async fn load_diva_config() -> std::io::Result<DivaConfig> {
    let mut cfg_dir = get_config_dir().await?;
    cfg_dir.push("rust4diva.toml");
    if !cfg_dir.exists() {
        let cfg = DivaConfig::new();
        return match toml::to_string(&cfg) {
            Ok(cfg_str) => {
                fs::write(cfg_dir, cfg_str).await?;
                Ok(cfg)
            }
            Err(e) => Err(std::io::Error::new(ErrorKind::Other, e.to_string())),
        };
    }
    if let Ok(cfg_str) = fs::read_to_string(cfg_dir).await {
        let res: Result<DivaConfig, _> = toml::from_str(cfg_str.as_str());
        match res {
            Ok(cfg) => {
                return Ok(cfg);
            }
            Err(e) => {
                eprintln!("{e}");
                return Err(std::io::Error::new(ErrorKind::Other, e.to_string()));
            }
        }
    }

    Ok(DivaConfig::new())
}

pub fn write_config_sync(cfg: DivaConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut target = get_config_dir_sync()?;
    target.push("rust4diva.toml");
    let cfg_str = toml::to_string(&cfg)?;
    Ok(std::fs::write(target, cfg_str)?)
}

pub async fn write_config(cfg: DivaConfig) -> std::io::Result<()> {
    let mut cfg_dir = get_config_dir_sync()?;
    cfg_dir.push("rust4diva.toml");
    return match toml::to_string(&cfg) {
        Ok(cfg_str) => {
            fs::write(cfg_dir, cfg_str).await?;
            Ok(())
        }
        Err(e) => Err(std::io::Error::new(ErrorKind::Other, e.to_string())),
    };
}

pub fn write_dml_config(dml: DivaModLoader) -> Result<(), Box<dyn Error + Send + Sync>> {
    if let Some(diva_dir) = get_diva_folder() {
        let mut target = PathBuf::from(diva_dir);
        target.push("config.toml");
        return match toml::to_string(&dml) {
            Ok(cfg_str) => Ok(std::fs::write(target, cfg_str)?),
            Err(e) => Err(std::io::Error::new(ErrorKind::Other, e.to_string()).into()),
        };
    }
    Err(Box::new(std::io::Error::new(
        ErrorKind::NotFound,
        "Unable to get diva directory",
    )))
}

pub static SETTINGS_OPEN: Mutex<bool> = Mutex::new(false);

pub async fn init_ui(diva_ui: &App, dark_tx: Sender<ColorScheme>) {
    let _ui_diva_dir_handle = diva_ui.as_weak();
    let main_ui_handle = diva_ui.as_weak();
    let scheme_handle = diva_ui.as_weak();
    let main_close_handle = main_ui_handle.clone();
    let dark_tx = dark_tx.clone();

    let weak = diva_ui.as_weak();
    diva_ui.global::<DivaLogic>().on_toggle_dml(move || {
        let weak = weak.clone();
        if let Ok(mut dml) = DML_CFG.try_lock() {
            dml.enabled = !dml.enabled;
            if let Ok(dml_str) = toml::to_string_pretty(&dml.clone()) {
                if let Some(diva_dir) = get_diva_folder() {
                    let mut buf = PathBuf::from(diva_dir);
                    buf.push("config.toml");
                    match std::fs::write(buf, dml_str) {
                        Ok(_) => {
                            weak.upgrade().unwrap().set_dml_enabled(dml.enabled.clone());
                        }
                        Err(e) => open_error_window(e.to_string()),
                    }
                }
            }
        }
    });

    diva_ui.global::<WindowLogic>().on_open_settings(move || {
        if let Ok(mut open) = SETTINGS_OPEN.try_lock() {
            if !open.clone() {
                *open = true;
                let dark_tx = dark_tx.clone();
                let current_scheme = scheme_handle.upgrade().unwrap().get_color_scheme();
                let steam_dir = get_steam_folder().unwrap_or("Not Set".to_string());
                let diva_dir = get_diva_folder().unwrap_or("Not Set".to_string());
                let settings = SettingsWindow::new().unwrap();
                // let settings = settings_weak.unwrap();

                settings.set_steam_dir(steam_dir.into());
                settings.set_diva_dir(diva_dir.into());
                settings.invoke_set_color_scheme(current_scheme);
                let main_ui = main_close_handle.unwrap();
                let close_handle = settings.as_weak();
                main_ui.on_close_windows(move || {
                    if let Some(ui) = close_handle.upgrade() {
                        ui.hide().unwrap();
                    }
                });

                let cancel_handle = settings.as_weak();
                settings.on_cancel(move || {
                    if let Ok(mut open) = SETTINGS_OPEN.try_lock() {
                        *open = false;
                    }
                    cancel_handle.unwrap().hide().unwrap();
                });

                settings.window().on_close_requested(|| {
                    if let Ok(mut open) = SETTINGS_OPEN.try_lock() {
                        *open = false;
                        return CloseRequestResponse::HideWindow;
                    }
                    CloseRequestResponse::KeepWindowShown
                });

                settings.show().unwrap();
                // settings.on
                let steam_handle = settings.as_weak();
                settings
                    .global::<SettingsLogic>()
                    .on_open_steam_picker(move |s| {
                        let steam_handle = steam_handle.clone();
                        let picker =
                            AsyncFileDialog::new().set_directory(PathBuf::from(s.to_string()));
                        tokio::spawn(async move {
                            match picker.pick_folder().await {
                                Some(dir) => {
                                    println!("{}", dir.path().display().to_string());
                                    let _ = steam_handle.upgrade_in_event_loop(move |ui| {
                                        ui.set_steam_dir(dir.path().display().to_string().into());
                                    });
                                }
                                None => {}
                            }
                        });
                    });

                let diva_dir_handle = settings.as_weak();

                settings
                    .global::<SettingsLogic>()
                    .on_open_diva_picker(move |s| {
                        let diva_dir_handle = diva_dir_handle.clone();
                        let picker =
                            AsyncFileDialog::new().set_directory(PathBuf::from(s.to_string()));
                        tokio::spawn(async move {
                            match picker.pick_folder().await {
                                Some(dir) => {
                                    println!("{}", dir.path().display().to_string());
                                    let _ = diva_dir_handle.upgrade_in_event_loop(move |ui| {
                                        ui.set_diva_dir(dir.path().display().to_string().into());
                                    });
                                }
                                None => {}
                            }
                        });
                    });

                let apply_handle = settings.as_weak();
                let color_handle = main_ui_handle.clone();
                settings
                    .global::<SettingsLogic>()
                    .on_apply_settings(move |settings| {
                        let dark_tx = dark_tx.clone();
                        let color_handle = color_handle.clone();
                        let apply_handle = apply_handle.clone();
                        if let Ok(mut cfg) = DIVA_CFG.lock() {
                            if let Ok(e) = PathBuf::from(settings.diva_dir.to_string()).try_exists()
                            {
                                if e {
                                    cfg.diva_dir = settings.diva_dir.to_string().clone();
                                } else {
                                    open_error_window("Invalid Project Diva Directory".to_string());
                                    return;
                                }
                            } else {
                                open_error_window("Invalid Project Diva Directory".to_string());
                                return;
                            }
                            if PathBuf::from(settings.steam_dir.to_string()).exists() {
                                cfg.steam_dir = settings.steam_dir.to_string().clone();
                            } else {
                                // open_error_window("Invalid Steam")
                            }
                            if PathBuf::from(settings.aft_dir.to_string()).exists() {
                                cfg.aft_dir = settings.aft_dir.to_string().clone();
                            }
                            cfg.aft_mode = settings.aft_mode;
                            cfg.dark_mode = settings.dark_mode;

                            let cfg = cfg.clone();
                            tokio::spawn(async move {
                                let cfg = cfg.clone();
                                match write_config(cfg.clone()).await {
                                    Ok(_) => {
                                        println!("Config successfully updated");
                                        let _ = apply_handle.upgrade_in_event_loop(move |ui| {
                                            if cfg.dark_mode {
                                                ui.invoke_set_color_scheme(ColorScheme::Dark);
                                            } else {
                                                ui.invoke_set_color_scheme(ColorScheme::Light);
                                            }
                                        });
                                        let _ = color_handle.upgrade_in_event_loop(move |ui| {
                                            if cfg.dark_mode {
                                                ui.invoke_set_color_scheme(ColorScheme::Dark);
                                            } else {
                                                ui.invoke_set_color_scheme(ColorScheme::Light);
                                            }
                                        });
                                        let _ = dark_tx.send(if cfg.dark_mode {
                                            ColorScheme::Dark
                                        } else {
                                            ColorScheme::Light
                                        });
                                    }
                                    Err(e) => {
                                        eprintln!("{e}");
                                        open_error_window(e.to_string());
                                    }
                                }
                            });
                        }
                    });
            }
        }
    });
}
