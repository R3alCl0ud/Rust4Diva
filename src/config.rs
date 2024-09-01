use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use rfd::AsyncFileDialog;
use serde::{Deserialize, Serialize};
use slint::{ComponentHandle, EventLoopError, Model, ModelRc, VecModel, Weak};
use tokio::fs;

use crate::diva::{get_diva_folder, get_steam_folder};
use crate::slint_generatedApp::App;

use crate::{diva::get_config_dir, SettingsLogic, SettingsWindow, WindowLogic, DIVA_CFG};

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

pub async fn write_config(cfg: DivaConfig) -> std::io::Result<()> {
    let mut cfg_dir = get_config_dir().await?;
    cfg_dir.push("rust4diva.toml");
    return match toml::to_string(&cfg) {
        Ok(cfg_str) => {
            fs::write(cfg_dir, cfg_str).await?;
            Ok(())
        }
        Err(e) => Err(std::io::Error::new(ErrorKind::Other, e.to_string())),
    };
}

pub async fn init_ui(diva_ui: &App) {
    let _ui_diva_dir_handle = diva_ui.as_weak();
    diva_ui.global::<WindowLogic>().on_open_settings(|| {
        let settings = SettingsWindow::new().unwrap();
        let steam_dir = get_steam_folder().unwrap_or("Not Set".to_string());
        let diva_dir = get_diva_folder().unwrap_or("Not Set".to_string());
        settings.set_steam_dir(steam_dir.into());
        settings.set_diva_dir(diva_dir.into());

        let cancel_handle = settings.as_weak();
        settings.on_cancel(move || {
            cancel_handle.upgrade().unwrap().hide().unwrap();
        });

        settings.show().unwrap();
        // settings.on
        let steam_handle = settings.as_weak();
        settings
            .global::<SettingsLogic>()
            .on_open_steam_picker(move |s| {
                let steam_handle = steam_handle.clone();
                let picker = AsyncFileDialog::new().set_directory(PathBuf::from(s.to_string()));
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
                let picker = AsyncFileDialog::new().set_directory(PathBuf::from(s.to_string()));
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
        settings
            .global::<SettingsLogic>()
            .on_apply_settings(|settings| {
                if let Ok(mut cfg) = DIVA_CFG.lock() {
                    if PathBuf::from(settings.diva_dir.to_string()).exists() {
                        cfg.diva_dir = settings.diva_dir.to_string().clone();
                    }
                    if PathBuf::from(settings.steam_dir.to_string()).exists() {
                        cfg.steam_dir = settings.steam_dir.to_string().clone();
                    }
                    if PathBuf::from(settings.aft_dir.to_string()).exists() {
                        cfg.aft_dir = settings.aft_dir.to_string().clone();
                    }
                    cfg.aft_mode = settings.aft_mode;
                    let cfg = cfg.clone();
                    tokio::spawn(async move {
                        match write_config(cfg.clone()).await {
                            Ok(_) => {
                                println!("Config successfully updated");
                            }
                            Err(e) => {
                                eprintln!("{e}");
                            }
                        }
                    });
                }
            });
    });
}
