use base64ct::{Base64, Encoding};
use filenamify::filenamify;
use sha2::{Digest, Sha256};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use sonic_rs::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::vec;
use tokio::fs;

use crate::config::{write_config, write_dml_config};
use crate::diva::{get_config_dir, get_diva_folder, open_error_window};
use crate::modmanagement::{get_mods_in_order, DivaMod};
use crate::slint_generatedApp::App;
use crate::{
    ConfirmDeletePack, DivaModElement, ModpackLogic, WindowLogic, DIVA_CFG, DML_CFG, MOD_PACKS,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModPack {
    pub name: String,
    pub mods: Vec<ModPackMod>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModPackMod {
    pub name: String,
    pub enabled: bool,
    #[serde(default)]
    pub path: String,
}

impl PartialEq for ModPackMod {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl PartialEq<DivaMod> for ModPackMod {
    fn eq(&self, other: &DivaMod) -> bool {
        self.name == other.config.name
    }
}

impl PartialEq<ModPackMod> for DivaMod {
    fn eq(&self, other: &ModPackMod) -> bool {
        self.config.name == other.name
    }
}

impl ModPackMod {
    pub fn to_element(self: &Self) -> DivaModElement {
        DivaModElement {
            author: SharedString::from(""),
            name: self.name.clone().into(),
            enabled: self.enabled,
            description: SharedString::from(""),
            version: SharedString::from(""),
            path: self.path.clone().into(),
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

impl ModPack {
    pub fn new(name: String) -> Self {
        Self {
            name: name.clone(),
            mods: Vec::new(),
        }
    }
}

pub async fn init(ui: &App) {
    let ui_add_mod_handle = ui.as_weak();
    let ui_remove_mod_handle = ui.as_weak();
    let ui_change_handle = ui.as_weak();
    let ui_add_pack_handle = ui.as_weak();
    let _ui_save_handle = ui.as_weak();
    let ui_apply_handle = ui.as_weak();
    let ui_delete_handle = ui.as_weak();

    match load_mod_packs().await {
        Ok(packs) => {
            let mut gpacks = MOD_PACKS.lock().unwrap();
            *gpacks = packs.clone();
            let binding = ui.get_modpacks();
            match binding.as_any().downcast_ref::<VecModel<SharedString>>() {
                None => {}
                Some(ui_packs) => {
                    ui_packs.push("All Mods".into());
                    for (pack, _mods) in gpacks.iter() {
                        ui_packs.push(pack.into());
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            open_error_window(e.to_string());
        }
    }

    ui.global::<ModpackLogic>().on_add_mod_to_pack(
        move |diva_mod_element: DivaModElement, _mod_pack| {
            let ui = ui_add_mod_handle.upgrade().unwrap();
            let pack_mods = ui.get_pack_mods();
            if let Some(pack_mods) = pack_mods
                .as_any()
                .downcast_ref::<VecModel<DivaModElement>>()
            {
                for element in pack_mods.iter() {
                    if element.name == diva_mod_element.name {
                        return;
                    }
                }
                println!("Pushing mod @ modpacks.rs 80");
                pack_mods.push(diva_mod_element);
            }
        },
    );

    ui.global::<ModpackLogic>().on_remove_mod_from_pack(
        move |diva_mod_element: DivaModElement, _mod_pack| {
            let ui = ui_remove_mod_handle.upgrade().unwrap();
            let pack_mods = ui.get_pack_mods();
            if let Some(pack_mods) = pack_mods
                .as_any()
                .downcast_ref::<VecModel<DivaModElement>>()
            {
                let filtered = pack_mods
                    .iter()
                    .filter(|item| item.name != diva_mod_element.name);
                let new_vec: Vec<DivaModElement> = filtered.collect();
                let vec_mod = VecModel::from(new_vec);
                ui.set_pack_mods(ModelRc::new(vec_mod));
            }
        },
    );

    ui.global::<ModpackLogic>()
        .on_change_modpack(move |mod_pack| {
            let ui_change_handle = ui_change_handle.clone();
            tokio::spawn(async move {
                if mod_pack.to_string() == "All Mods" || mod_pack.to_string() == "" {
                    let _ = ui_change_handle.upgrade_in_event_loop(|ui| {
                        ui.set_active_pack("".into());
                        let vec = VecModel::<DivaModElement>::default();
                        for m in get_mods_in_order() {
                            vec.push(m.into());
                        }
                        let model = ModelRc::new(vec);
                        ui.set_pack_mods(model.clone());
                        ui.global::<ModpackLogic>().invoke_apply_modpack(model);
                    });
                }
                if let Ok(packs) = MOD_PACKS.lock() {
                    let pack = packs.get(&mod_pack.clone().to_string()).clone();
                    if pack.is_none() {
                        return;
                    }
                    let pack = pack.unwrap();
                    let pack = pack.clone();
                    let _ = ui_change_handle.upgrade_in_event_loop(move |ui| {
                        let pack_mods: VecModel<DivaModElement> = VecModel::default();
                        for module in pack.mods.clone() {
                            pack_mods.push(module.to_element());
                        }
                        let model = ModelRc::new(pack_mods);
                        ui.set_pack_mods(model.clone());
                        ui.set_active_pack(pack.name.into());
                        ui.global::<ModpackLogic>().invoke_apply_modpack(model);
                    });
                }
            });
        });

    ui.global::<ModpackLogic>().on_create_new_pack(move |pack| {
        println!("We even get called @ modpack.rs 185");

        let pack_name = filenamify(pack.to_string());

        if pack_name.len() <= 0 || pack_name == "All Mods" {
            return;
        }
        let modpack = ModPack::new(pack_name.clone());
        let mut gpacks = MOD_PACKS.lock().unwrap();
        if !gpacks.contains_key(&pack_name) {
            gpacks.insert(pack_name.clone(), modpack);
            let ui = ui_add_pack_handle.upgrade().unwrap();
            let binding = ui.get_modpacks();
            match binding.as_any().downcast_ref::<VecModel<SharedString>>() {
                Some(f) => f.push(pack_name.into()),
                None => {}
            }
        }
    });

    ui.global::<ModpackLogic>()
        .on_save_modpack(move |modpack, mods| {
            let modpack = modpack.to_string();
            let modpack = modpack.clone();
            match mods.as_any().downcast_ref::<VecModel<DivaModElement>>() {
                Some(mods) => {
                    let mut vec_mods: Vec<ModPackMod> = Vec::new();
                    for m in mods.iter() {
                        vec_mods.push(m.to_packmod());
                    }
                    {
                        let mut gpacks = MOD_PACKS.lock().unwrap();
                        match gpacks.get_mut(&modpack) {
                            Some(pack) => {
                                pack.mods = vec_mods.clone();
                                match sonic_rs::to_string_pretty(&pack.clone()) {
                                    Ok(pack_str) => {
                                        tokio::spawn(async move {
                                            let mut buf = get_modpacks_folder()
                                                .await
                                                .unwrap_or(PathBuf::new());
                                            buf.push(format!("{modpack}.json"));
                                            match fs::write(buf, pack_str).await {
                                                Ok(_) => {}
                                                Err(e) => {
                                                    eprintln!("{e}");

                                                    let msg = format!(
                                                        "Unable to save modpack: \n{}",
                                                        e.to_string()
                                                    );
                                                    open_error_window(msg);
                                                }
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        let msg =
                                            format!("Unable to save modpack: \n{}", e.to_string());
                                        open_error_window(msg);
                                    }
                                }
                            }
                            None => {}
                        }
                    }
                }
                None => {
                    return;
                }
            }
        });

    ui.global::<ModpackLogic>().on_apply_modpack(move |mods| {
        match mods.as_any().downcast_ref::<VecModel<DivaModElement>>() {
            Some(mods) => {
                let mut vec_mods: Vec<String> = Vec::new();
                for m in mods.iter() {
                    if let Some(dir) = m.dir_name() {
                        vec_mods.push(dir);
                    }
                }
                if let Ok(mut cfg) = DIVA_CFG.try_lock() {
                    let ui = ui_apply_handle.upgrade().unwrap();
                    if vec_mods.is_empty() {
                        vec_mods = cfg.priority.clone();
                        cfg.applied_pack = "".to_string();
                    } else {
                        cfg.applied_pack = ui.get_current_pack().to_string();
                    }
                    ui.set_active_pack(cfg.applied_pack.clone().into());
                    let cfg = cfg.clone();

                    tokio::spawn(async move {
                        if let Err(e) = write_config(cfg.clone()).await {
                            open_error_window(e.to_string());
                        }
                    });
                }
                tokio::spawn(async move {
                    if let Ok(mut dml) = DML_CFG.lock() {
                        dml.priority = vec_mods.clone();
                        if let Ok(dmlcfg) = toml::to_string(&dml.clone()) {
                            if let Some(dir) = get_diva_folder() {
                                let mut buf = PathBuf::from(dir);
                                buf.push("config.toml");
                                if buf.exists() {
                                    match std::fs::write(buf, dmlcfg) {
                                        Ok(_) => {
                                            println!("Mod pack successfully applied");
                                        }
                                        Err(e) => {
                                            eprintln!("{e}");
                                            let msg = format!(
                                                "Unable to activate modpack: \n{}",
                                                e.to_string()
                                            );
                                            open_error_window(msg);
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
            }
            None => {}
        }
    });
    let scheme_handle = ui.as_weak();
    ui.global::<WindowLogic>()
        .on_open_delete_dialog(move |to_delete| {
            let ui_delete_handle = ui_delete_handle.clone();
            let current_scheme = scheme_handle.upgrade().unwrap().get_color_scheme();
            let delete = ConfirmDeletePack::new().unwrap();
            delete.invoke_set_color_scheme(current_scheme);
            delete.set_modpack(to_delete);
            delete.show().unwrap();
            let close_handle = delete.as_weak();
            delete.on_close(move || {
                close_handle.upgrade().unwrap().hide().unwrap();
            });
            delete
                .global::<ModpackLogic>()
                .on_delete_modpack(move |packname| {
                    println!("{:?}", packname.to_string());
                    if let Ok(mut packs) = MOD_PACKS.lock() {
                        match packs.remove(&packname.to_string()) {
                            Some(pack) => {
                                let mut packsvec: Vec<ModPack> = vec![];
                                for p in packs.values() {
                                    packsvec.push(p.clone());
                                }
                                let ui_delete_handle = ui_delete_handle.clone();
                                tokio::spawn(async move {
                                    let mut buf =
                                        get_modpacks_folder().await.unwrap_or(PathBuf::new());
                                    buf.push(format!("{}.json", filenamify(pack.name)));
                                    match fs::remove_file(buf).await {
                                        Ok(_) => {
                                            let _ = ui_delete_handle.clone().upgrade_in_event_loop(
                                                move |ui| {
                                                    let vec_mod: VecModel<SharedString> =
                                                        VecModel::default();
                                                    for p in packsvec {
                                                        vec_mod.push(p.name.clone().into());
                                                    }
                                                    ui.set_modpacks(ModelRc::new(vec_mod));
                                                },
                                            );
                                        }
                                        Err(e) => {
                                            let msg = format!(
                                                "Unable to delete modpack: \n{}",
                                                e.to_string()
                                            );
                                            open_error_window(msg);
                                        }
                                    }
                                });
                            }
                            None => {}
                        }
                    }
                });
        });

    if let Ok(cfg) = DIVA_CFG.try_lock() {
        // println!();
        ui.set_active_pack(cfg.applied_pack.clone().into());
        ui.global::<ModpackLogic>()
            .invoke_change_modpack(cfg.applied_pack.clone().into());
    }
}

pub async fn load_mod_packs() -> std::io::Result<HashMap<String, ModPack>> {
    let packs_dir = get_modpacks_folder().await;
    if packs_dir.is_err() {
        return Err(packs_dir.unwrap_err().into());
    }
    let mut packs: HashMap<String, ModPack> = HashMap::new();
    let packs_dir = packs_dir.unwrap();
    for entry in std::fs::read_dir(packs_dir.clone())? {
        let entry = entry?;
        if entry.path().is_dir() {
            continue;
        }
        let fuck = fs::read_to_string(entry.path()).await?;
        let pack: ModPack = sonic_rs::from_str(fuck.as_str())?;
        packs.insert(pack.name.clone(), pack);
    }

    Ok(packs)
}

pub async fn get_modpacks_folder() -> std::io::Result<PathBuf> {
    match get_config_dir().await {
        Ok(mut config_dir) => {
            if let Ok(cfg) = DIVA_CFG.try_lock() {
                let cfg = cfg.clone();
                config_dir.push(hash_dir_name(cfg.diva_dir.clone()));
                if !config_dir.exists() {
                    std::fs::create_dir(config_dir.clone())?
                }
                config_dir.push("modpacks");
                if !config_dir.exists() {
                    std::fs::create_dir(config_dir.clone())?
                }
            }
            Ok(config_dir)
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn save_modpack(pack: ModPack) -> std::io::Result<()> {
    match get_modpacks_folder().await {
        Ok(mut packs_dir) => {
            packs_dir.push(filenamify(pack.name.clone()) + ".json");
            if let Ok(pckstr) = sonic_rs::to_string_pretty(&pack.clone()) {
                return fs::write(packs_dir, pckstr).await;
            }
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn apply_mod_priority() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Ok(cfg) = DIVA_CFG.try_lock() {
        let mut prio = vec![];
        if cfg.applied_pack != "".to_owned() {
            if let Ok(packs) = MOD_PACKS.try_lock() {
                if let Some(current_pack) = packs.get(&cfg.applied_pack) {
                    for m in current_pack.clone().mods {
                        prio.push(m.clone().dir_name().unwrap_or(m.name));
                    }
                }
            }
        } else {
            prio = cfg.priority.clone();
        }
        if let Ok(mut dml) = DML_CFG.try_lock() {
            dml.priority = prio;
            return Ok(write_dml_config(dml.clone())?);
        }
    }
    Ok(())
}

pub fn hash_dir_name(dir: String) -> String {
    let hash = Sha256::digest(dir);
    Base64::encode_string(&hash)
}
