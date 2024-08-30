use std::io::ErrorKind;

use serde::{Deserialize, Serialize};
use slint::{ComponentHandle, EventLoopError, Model, ModelRc, VecModel, Weak};
use tokio::fs;

use crate::diva::{get_diva_folder, get_steam_folder};
use crate::slint_generatedApp::App;

use crate::{diva::get_config_dir, SettingsWindow, WindowLogic, DIVA_CFG};

#[derive(Deserialize, Serialize, Clone)]
pub struct DivaConfig {
    /// this is the global priority order, this is used when no modpack is applied
    pub priority: Vec<String>,
    pub diva_dir: String,
    #[serde(default)]
    pub steam_dir: String,
    #[serde(default)]
    pub applied_pack: String, // pub
}

impl DivaConfig {
    pub fn new() -> Self {
        Self {
            priority: vec![],
            diva_dir: "".to_string(),
            steam_dir: "".to_string(),
            applied_pack: "".to_string(),
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

pub async fn init_ui(app: &App) {
    let _ui_diva_dir_handle = app.as_weak();
    app.global::<WindowLogic>().on_open_settings(|| {
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
    });
}
