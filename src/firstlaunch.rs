use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use crate::diva::{get_diva_folder, open_error_window};
use crate::slint_generatedApp::App;
use crate::{FirstSetup, Loadout, SetupLogic, DIVA_CFG};
use rfd::AsyncFileDialog;
use serde::{Deserialize, Serialize};
use slint::private_unstable_api::re_exports::ColorScheme;
use slint::{ModelRc, VecModel};
use slint_interpreter::ComponentHandle;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DmmConfig {
    #[serde(rename(serialize = "CurrentGame", deserialize = "CurrentGame"))]
    current_game: String,
    #[serde(rename(serialize = "Configs", deserialize = "Configs"))]
    configs: HashMap<String, DmmPDMMConfig>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DmmPDMMConfig {
    #[serde(rename(serialize = "Launcher", deserialize = "Launcher"), default)]
    launcher: Option<String>,
    #[serde(rename(serialize = "GamePath", deserialize = "GamePath"), default)]
    game_path: Option<String>,
    #[serde(rename(serialize = "ModsFolder", deserialize = "ModsFolder"), default)]
    mods_folder: Option<String>,
    #[serde(rename(serialize = "Loadouts", deserialize = "Loadouts"), default)]
    loadouts: HashMap<String, Vec<DmmLoadoutMod>>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DmmLoadoutMod {
    name: String,
    enabled: bool,
}

pub static DMM_CFG: LazyLock<Mutex<Option<DmmConfig>>> = LazyLock::new(|| Mutex::new(None));

pub async fn init(_diva_ui: &App) -> Result<(), slint::PlatformError> {
    let diva_dir = get_diva_folder();
    if let Ok(cfg) = DIVA_CFG.lock() {
        if cfg.first_run {
            let setup = FirstSetup::new()?;
            if cfg.dark_mode {
                setup.invoke_set_color_scheme(ColorScheme::Dark);
            }
            if !cfg.dark_mode {
                setup.invoke_set_color_scheme(ColorScheme::Light);
            }
            if let Some(diva_dir) = diva_dir {
                setup.set_diva_dir(diva_dir.into());
            }
            let import_handle = setup.as_weak();
            setup.global::<SetupLogic>().on_import_dmm(move || {
                let import_handle = import_handle.clone();
                let picker = AsyncFileDialog::new();
                tokio::spawn(async move {
                    if let Some(dmm_dir) = picker.pick_folder().await {
                        let mut buf = PathBuf::from(dmm_dir.path());
                        buf.push("Config.json");
                        if buf.exists() {
                            if let Ok(cfgstr) = fs::read_to_string(buf) {
                                match sonic_rs::from_str::<DmmConfig>(cfgstr.as_str()) {
                                    Ok(cfg) => {
                                        if let Ok(mut dmmcfg) = DMM_CFG.try_lock() {
                                            *dmmcfg = Some(cfg.clone());
                                        }
                                        if let Some(pdmm) =
                                            cfg.configs.get(&"Project DIVA Mega Mix+".to_string())
                                        {
                                            if let Some(mods_dir) = pdmm.mods_folder.clone() {
                                                println!("{}", mods_dir);
                                                let mut mbuf = PathBuf::from(mods_dir);
                                                mbuf.pop();
                                                if mbuf.exists() {
                                                    let _ = import_handle.upgrade_in_event_loop(
                                                        move |ui| {
                                                            ui.set_diva_dir(
                                                                mbuf.display().to_string().into(),
                                                            );
                                                        },
                                                    );
                                                }
                                            }
                                            let mut loadouts: Vec<Loadout> = Default::default();
                                            for (loadout, _mods) in pdmm.loadouts.iter() {
                                                println!("Loadout found: {}", loadout);
                                                loadouts.push(Loadout {
                                                    name: loadout.clone().into(),
                                                    import: true,
                                                });
                                            }
                                            let _ =
                                                import_handle.upgrade_in_event_loop(move |ui| {
                                                    ui.set_loadouts(ModelRc::new(VecModel::from(
                                                        loadouts,
                                                    )));
                                                });
                                        }
                                    }
                                    Err(e) => {
                                        open_error_window(e.to_string());
                                    }
                                }
                            }
                        }
                    }
                });
            });
            let apply_handle = setup.as_weak();
            setup.global::<SetupLogic>().on_apply(move || {
                let ui = apply_handle.upgrade().unwrap();
                let mut diva_buf = PathBuf::from(ui.get_diva_dir().to_string());
                println!("PDMM: {}", diva_buf.display());
                if diva_buf.exists() && diva_buf.is_dir() {
                    diva_buf.push("DivaMegaMix.exe");
                    if !diva_buf.exists() {
                        open_error_window(
                            "Selected Directory does not contain DivaMegaMix.exe".to_string(),
                        );
                        return;
                    }
                    diva_buf.pop();
                } else {
                    open_error_window(
                        "Selected Project Diva directory does not exist or is a file".to_string(),
                    );
                    return;
                }
                ui.hide().unwrap();
            });
            setup.show()?;
        }
    }

    Ok(())
}
