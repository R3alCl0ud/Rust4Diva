use std::error::Error;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Mutex;

use rfd::AsyncFileDialog;
use serde::{Deserialize, Serialize};
use slint::private_unstable_api::re_exports::ColorScheme;
use slint::{CloseRequestResponse, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use tokio::fs;
use tokio::sync::broadcast::Sender;

use crate::diva::{
    find_diva_folder, get_config_dir_sync, get_diva_folder, get_steam_folder, open_error_window,
};
use crate::modmanagement::{get_mods_in_order, load_mods, set_mods_table, DivaModLoader};
use crate::modpacks::{load_mod_packs, ModPackMod};
use crate::slint_generatedApp::App;
use crate::{DML_CFG, MOD_PACKS};

use crate::{diva::get_config_dir, DivaLogic, SettingsLogic, SettingsWindow, WindowLogic, R4D_CFG};

#[derive(Deserialize, Serialize, Clone)]
pub struct OldDivaConfig {
    /// this is the global priority order, this is used when no modpack is applied
    pub priority: Vec<String>,
    pub diva_dir: String,
    #[serde(default)]
    pub steam_dir: String,
    #[serde(default)]
    pub aft_dir: String,
    #[serde(default)]
    pub applied_pack: String,
    #[serde(default)]
    pub aft_mode: bool,
    #[serde(default = "yes")]
    pub dark_mode: bool,
    #[serde(default = "yes")]
    pub first_run: bool,
    #[serde(default)]
    pub dml_version: String,
    #[serde(default)]
    pub diva_dirs: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct DivaConfig {
    /// this is the global priority order, this is used when no modpack is applied
    pub priority: Vec<ModPackMod>,
    pub diva_dir: String,
    #[serde(default)]
    pub steam_dir: String,
    #[serde(default)]
    pub aft_dir: String,
    #[serde(default)]
    pub applied_pack: String,
    #[serde(default)]
    pub aft_mode: bool,
    #[serde(default = "yes")]
    pub use_system_theme: bool,
    #[serde(default = "yes")]
    pub use_system_scaling: bool,
    #[serde(default)]
    pub scale: f32,
    #[serde(default = "yes")]
    pub dark_mode: bool,
    #[serde(default = "yes")]
    pub first_run: bool,
    #[serde(default)]
    pub dml_version: String,
    #[serde(default)]
    pub diva_dirs: Vec<String>,
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
            diva_dirs: vec![],
            use_system_theme: true,
            use_system_scaling: true,
            scale: 1.0,
        }
    }
}

impl From<OldDivaConfig> for DivaConfig {
    fn from(value: OldDivaConfig) -> Self {
        let mut prio = vec![];
        let diva_dir = value.diva_dir.clone();
        for p in value.priority {
            let mut buf = PathBuf::from(diva_dir.clone());
            buf.push(p);
            if buf.exists() {
                prio.push(ModPackMod {
                    name: "".to_owned(),
                    enabled: true,
                    path: buf.to_str().unwrap().to_owned(),
                });
            }
        }

        Self {
            priority: prio,
            diva_dir: value.diva_dir.clone(),
            steam_dir: value.steam_dir.clone(),
            aft_dir: value.aft_dir.clone(),
            applied_pack: value.applied_pack.clone(),
            aft_mode: value.aft_mode.clone(),
            dark_mode: value.dark_mode.clone(),
            first_run: value.first_run.clone(),
            dml_version: value.dml_version.clone(),
            diva_dirs: value.diva_dirs.clone(),
            use_system_theme: true,
            use_system_scaling: true,
            scale: 1.0,
        }
    }
}

fn yes() -> bool {
    true
}

impl OldDivaConfig {
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
            diva_dirs: vec![],
        }
    }
}

pub async fn load_diva_config() -> std::io::Result<DivaConfig> {
    let mut cfg_dir = get_config_dir()?;
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
        match toml::from_str::<DivaConfig>(&cfg_str) {
            Ok(mut cfg) => {
                if cfg.diva_dirs.is_empty() && !cfg.diva_dir.is_empty() {
                    cfg.diva_dirs.push(cfg.diva_dir.clone());
                }

                return Ok(cfg);
            }
            Err(e) => {
                println!("Failed to parse config\nTrying to update config to new version");
                if let Ok(cfg) = toml::from_str::<OldDivaConfig>(&cfg_str) {
                    println!("Config Updated");
                    return Ok(cfg.into());
                }
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
        if !diva_dir.is_empty() {
            let mut target = PathBuf::from(diva_dir.clone());
            target.push("config.toml");
            return match toml::to_string(&dml) {
                Ok(cfg_str) => Ok(std::fs::write(target, cfg_str)?),
                Err(e) => Err(std::io::Error::new(ErrorKind::Other, e.to_string()).into()),
            };
        }
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
                if let Some(diva_dir) = find_diva_folder() {
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
                let diva_dir = find_diva_folder().unwrap_or("Not Set".to_string());
                let settings = SettingsWindow::new().unwrap();
                // let settings = settings_weak.unwrap();

                settings.set_steam_dir(steam_dir.into());
                settings.set_diva_dir(diva_dir.into());
                settings.invoke_set_color_scheme(current_scheme);

                if let Ok(cfg) = R4D_CFG.try_lock() {
                    let vec = VecModel::<SharedString>::default();
                    for dir in cfg.diva_dirs.clone() {
                        vec.push(dir.into());
                    }

                    if vec.row_count() == 0 {
                        vec.push("/path/to/pdx".into());
                    }
                    settings.set_pdmm_dirs(ModelRc::new(vec));
                    settings.set_active_pdmm(cfg.diva_dir.clone().into());
                    settings.set_b_system_theme(cfg.use_system_theme);
                    settings.set_b_system_scale(cfg.use_system_scaling);
                    settings.set_b_dark_theme(cfg.dark_mode);
                    settings.set_f_scale(cfg.scale);
                }

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
                    .on_open_diva_picker(move |s, row| {
                        let diva_dir_handle = diva_dir_handle.clone();
                        let picker =
                            AsyncFileDialog::new().set_directory(PathBuf::from(s.to_string()));
                        tokio::spawn(async move {
                            match picker.pick_folder().await {
                                Some(dir) => {
                                    println!("{}", dir.path().display().to_string());
                                    let _ = diva_dir_handle.upgrade_in_event_loop(move |ui| {
                                        let model = ui.get_pdmm_dirs();
                                        model.set_row_data(
                                            row as usize,
                                            dir.path().display().to_string().into(),
                                        );
                                    });
                                }
                                None => {}
                            }
                        });
                    });

                let weak = settings.as_weak();
                settings
                    .global::<SettingsLogic>()
                    .on_add_pdmm_location(move || {
                        let settings = weak.unwrap();
                        let model = settings.get_pdmm_dirs();
                        match model.as_any().downcast_ref::<VecModel<SharedString>>() {
                            Some(vec) => vec.push("/path/to/pdx".into()),
                            None => todo!(),
                        }
                    });
                let weak = settings.as_weak();

                settings
                    .global::<SettingsLogic>()
                    .on_remove_pdmm_location(move |idx| {
                        let settings = weak.unwrap();
                        let model = settings.get_pdmm_dirs();
                        match model.as_any().downcast_ref::<VecModel<SharedString>>() {
                            Some(vec) => {
                                vec.remove(idx as usize);
                            }
                            None => {}
                        }
                    });

                let apply_handle = settings.as_weak();
                let color_handle = main_ui_handle.clone();
                settings
                    .global::<SettingsLogic>()
                    .on_apply_settings(move |settings| {
                        let dark_tx = dark_tx.clone();
                        let color_handle = color_handle.clone();
                        let apply_handle = apply_handle.clone();
                        let mut lcfg = None;
                        if let Ok(mut cfg) = R4D_CFG.lock() {
                            let mut dirs = vec![];
                            for dir in settings.diva_dirs.iter() {
                                let mut buf = PathBuf::from(dir.clone().to_string());
                                buf.push("DivaMegaMix.exe");
                                if let Ok(e) = buf.try_exists() {
                                    if e {
                                        dirs.push(dir.to_string());
                                    } else {
                                        open_error_window(
                                            "Invalid Project Diva Directory".to_string(),
                                        );
                                        return;
                                    }
                                } else {
                                    open_error_window("Invalid Project Diva Directory".to_string());
                                    return;
                                }
                            }
                            if PathBuf::from(settings.steam_dir.to_string()).exists() {
                                cfg.steam_dir = settings.steam_dir.to_string().clone();
                            }

                            cfg.diva_dir = settings.diva_dir.to_string();
                            cfg.diva_dirs = dirs;
                            cfg.aft_mode = settings.aft_mode;
                            cfg.dark_mode = settings.dark_mode;
                            cfg.use_system_scaling = settings.system_scale;
                            cfg.use_system_theme = settings.system_theme;
                            cfg.scale = settings.scale.clamp(0.1, 10.0);
                            lcfg = Some(cfg.clone());
                        }
                        if let Some(cfg) = lcfg {
                            tokio::spawn(async move {
                                let cfg = cfg.clone();
                                match write_config(cfg.clone()).await {
                                    Ok(_) => {
                                        println!("Config successfully updated");
                                        let _ =
                                            apply_handle.clone().upgrade_in_event_loop(move |ui| {
                                                if cfg.use_system_theme {
                                                    ui.invoke_set_color_scheme(
                                                        ColorScheme::Unknown,
                                                    );
                                                } else if cfg.dark_mode {
                                                    ui.invoke_set_color_scheme(ColorScheme::Dark);
                                                } else {
                                                    ui.invoke_set_color_scheme(ColorScheme::Light);
                                                }
                                            });
                                        let _ =
                                            color_handle.clone().upgrade_in_event_loop(move |ui| {
                                                if cfg.use_system_theme {
                                                    ui.invoke_set_color_scheme(
                                                        ColorScheme::Unknown,
                                                    );
                                                } else if cfg.dark_mode {
                                                    ui.invoke_set_color_scheme(ColorScheme::Dark);
                                                } else {
                                                    ui.invoke_set_color_scheme(ColorScheme::Light);
                                                }
                                            });
                                        let _ = dark_tx.send(if cfg.use_system_theme {
                                            ColorScheme::Unknown
                                        } else {
                                            if cfg.dark_mode {
                                                ColorScheme::Dark
                                            } else {
                                                ColorScheme::Light
                                            }
                                        });
                                        if load_mods().is_ok() {
                                            let _ = set_mods_table(
                                                &get_mods_in_order(),
                                                color_handle.clone(),
                                            );
                                        }

                                        if let Ok(packs) = load_mod_packs().await {
                                            let _ = color_handle.clone().upgrade_in_event_loop(
                                                move |ui| {
                                                    let mut gpacks = MOD_PACKS.lock().unwrap();
                                                    *gpacks = packs.clone();
                                                    let ui_packs =
                                                        VecModel::<SharedString>::default();
                                                    ui_packs.push("All Mods".into());
                                                    for (pack, _mods) in gpacks.iter() {
                                                        ui_packs.push(pack.into());
                                                    }
                                                    ui.set_modpacks(ModelRc::new(ui_packs));
                                                },
                                            );
                                        }
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
