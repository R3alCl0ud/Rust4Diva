use base64ct::{Base64, Encoding};
use filenamify::filenamify;
use sha2::{Digest, Sha256};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use sonic_rs::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::vec;
use tokio::fs;
use toml_edit::value;

use crate::config::{write_config, write_config_sync, write_dml_config};
use crate::diva::{get_config_dir, get_diva_folder, open_error_window};
use crate::modmanagement::{get_mods_in_order, save_mod_config, DivaMod};
use crate::slint_generatedApp::App;
use crate::{
    ConfirmDeletePack, DivaModElement, ModpackLogic, WindowLogic, DML_CFG, MODS, MOD_PACKS, R4D_CFG,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModPack {
    pub name: String,
    pub mods: Vec<ModPackMod>,
}

// impl ModPAc

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModPackMod {
    pub name: String,
    pub enabled: bool,
    #[serde(default)]
    pub path: String,
}

impl PartialEq for ModPackMod {
    fn eq(&self, other: &Self) -> bool {
        let left = match self.dir_name() {
            Some(name) => name,
            None => self.name.clone(),
        };
        let right = match other.dir_name() {
            Some(name) => name,
            None => other.name.clone(),
        };
        left == right
    }
}

impl PartialEq<DivaMod> for ModPackMod {
    fn eq(&self, other: &DivaMod) -> bool {
        let left = match self.dir_name() {
            Some(name) => name,
            None => self.name.clone(),
        };
        let right = match other.dir_name() {
            Some(name) => name,
            None => other.config["name"].as_str().unwrap().to_string(),
        };
        left == right
    }
}

impl PartialEq<ModPackMod> for DivaMod {
    fn eq(&self, other: &ModPackMod) -> bool {
        let left = match self.dir_name() {
            Some(name) => name,
            None => self.config["name"].as_str().unwrap().to_string(),
        };
        let right = match other.dir_name() {
            Some(name) => name,
            None => other.name.clone(),
        };
        left == right
    }
}

impl ModPackMod {
    pub fn to_element(self: &Self) -> DivaModElement {
        if let Ok(mods) = MODS.try_lock() {
            if let Some(m) = mods.get(&self.dir_name().unwrap_or_default()) {
                return m.clone().into();
            }
        }
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

            let mut vec: Vec<SharedString> = vec![];
            for (pack, _mods) in gpacks.iter() {
                vec.push(pack.into());
            }
            vec.sort_by_key(|s| s.to_lowercase());

            vec.insert(0, "All Mods".into());
            ui.set_modpacks(ModelRc::new(VecModel::from(vec.clone())));
            if let Ok(cfg) = R4D_CFG.try_lock() {
                if cfg.applied_pack != "All Mods" && cfg.applied_pack != "" {
                    if let Some(idx) = vec.iter().position(|p| p.to_string() == cfg.applied_pack) {
                        #[cfg(debug_assertions)]
                        println!("idx: {idx}");
                        ui.set_current_pack_idx(idx as i32);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            open_error_window(e.to_string());
        }
    }

    ui.global::<ModpackLogic>()
        .on_add_mod_to_pack(move |to_add: DivaModElement, mod_pack| {
            let ui = ui_add_mod_handle.upgrade().unwrap();
            let model = ui.get_pack_mods();
            if let Some(pack_mods) = model.as_any().downcast_ref::<VecModel<DivaModElement>>() {
                if pack_mods.iter().find(|e| e.is_same_as(&to_add)).is_some() {
                    return;
                }
                println!("Pushing mod @ modpacks.rs 80");
                pack_mods.push(to_add);
                let vec = VecModel::default();
                for m in pack_mods.iter() {
                    vec.push(m);
                }
                ui.global::<ModpackLogic>()
                    .invoke_save_modpack(mod_pack, ModelRc::new(vec));
            }
        });

    ui.global::<ModpackLogic>().on_remove_mod_from_pack(
        move |to_remove: DivaModElement, mod_pack| {
            let ui = ui_remove_mod_handle.upgrade().unwrap();
            let pack_mods = ui.get_pack_mods();
            if let Some(pack_mods) = pack_mods
                .as_any()
                .downcast_ref::<VecModel<DivaModElement>>()
            {
                let filtered = pack_mods.iter().filter(|item| !item.is_same_as(&to_remove));
                let new_vec: Vec<DivaModElement> = filtered.collect();
                let vec_mod = VecModel::from(new_vec);
                let model = ModelRc::new(vec_mod);
                ui.set_pack_mods(model.clone());
                ui.global::<ModpackLogic>()
                    .invoke_save_modpack(mod_pack, model);
            }
        },
    );

    let weak = ui.as_weak();
    ui.global::<ModpackLogic>().on_set_search(move |term| {
        let term = term.to_string();
        let mods = get_mods_in_order();
        let filtered: Vec<DivaModElement> = mods
            .iter()
            .cloned()
            .filter(|m| m.search(&term))
            .map(|m| m.into())
            .collect();
        let vecmod = VecModel::from(filtered);
        weak.unwrap().set_pack_mods(ModelRc::new(vecmod));
    });

    ui.global::<ModpackLogic>()
        .on_change_modpack(move |mod_pack| {
            #[cfg(debug_assertions)]
            println!("Change Modpack called for: {mod_pack}");
            let ui_change_handle = ui_change_handle.clone();
            // make sure that the mutex is unlocked after we are done with it
            let mut pack = ModPack::new("All Mods".to_owned());
            #[cfg(debug_assertions)]
            println!("Locking CFG @ modpacks.rs::on_change_modpack()");
            {
                let mut cfg = match R4D_CFG.try_lock() {
                    Ok(cfg) => cfg,
                    _ => {
                        println!("Failed to lock");
                        return;
                    }
                };
                cfg.applied_pack = mod_pack.to_string();
                pack.mods = cfg.priority.clone();
                if write_config_sync(cfg.clone()).is_err() {
                    return;
                }
            }
            #[cfg(debug_assertions)]
            println!("Unlocked CFG @ modpacks.rs::on_change_modpack()");

            let mut mods = get_mods_in_order();
            // make sure that the mutex is unlocked after we are done with it
            #[cfg(debug_assertions)]
            println!("Locking MOD_PACKS @ modpacks.rs::on_change_modpack()");
            {
                let mut packs = match MOD_PACKS.try_lock() {
                    Ok(packs) => packs,
                    Err(_) => return,
                };
                #[cfg(debug_assertions)]
                println!("Locking MODS @ modpacks.rs::on_change_modpack()");

                let mut gmods = match MODS.try_lock() {
                    Ok(ms) => ms,
                    Err(_) => return,
                };
                match packs.get_mut(&mod_pack.to_string()) {
                    Some(p) => {
                        if p.mods.len() != mods.len() {
                            p.mods = mods.iter().map(|m| ModPackMod::from(m.clone())).collect();
                            match save_modpack_sync(p.clone()) {
                                Err(e) => eprintln!("{e}"),
                                _ => {}
                            }
                        }
                        pack = p.clone();
                    }
                    None => {}
                }
                for m in mods.iter_mut() {
                    if let Some(pm) = pack.mods.iter().find(|p| p.path == m.path) {
                        if m.config["enabled"].as_bool().unwrap() != pm.enabled {
                            m.config["enabled"] = value(pm.enabled);
                            if let Err(e) =
                                save_mod_config(PathBuf::from(m.path.clone()), &m.config)
                            {
                                eprintln!("{e}");
                            }
                            gmods.insert(m.dir_name().unwrap(), m.clone());
                        }
                    }
                }
            }
            #[cfg(debug_assertions)]
            println!("Unlocked MOD_PACKS @ modpacks.rs::on_change_modpack()");
            #[cfg(debug_assertions)]
            println!("Unlocked MODS @ modpacks.rs::on_change_modpack()");
            // actually create the model now
            let vec: VecModel<DivaModElement> = Default::default();
            for m in mods {
                vec.push(m.into());
            }
            let ui = ui_change_handle.unwrap();
            let model = ModelRc::new(vec);
            ui.set_pack_mods(model.clone());
            ui.set_active_pack(mod_pack.clone());
            let packs = ui.get_modpacks();
            if let Some(idx) = packs
                .iter()
                .position(|p| p.to_string() == mod_pack.to_string())
            {
                ui.set_current_pack_idx(idx as i32);
            }

            ui.global::<ModpackLogic>().invoke_apply_modpack(model);
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
    let weak = ui.as_weak();
    ui.global::<ModpackLogic>()
        .on_save_modpack(move |pack_name, mods| {
            let pack_name = pack_name.to_string();
            let vecmods = match mods.as_any().downcast_ref::<VecModel<DivaModElement>>() {
                Some(mods) => mods,
                None => &VecModel::default(),
            };
            let mut vec = vec![];
            for m in vecmods.iter() {
                vec.push(m.to_packmod());
            }
            let mut packs = match MOD_PACKS.try_lock() {
                Ok(packs) => packs,
                Err(_) => return,
            };
            let modpack = match packs.get_mut(&pack_name) {
                Some(pack) => pack,
                None => return,
            };
            modpack.mods = vec;
            match save_modpack_sync(modpack.clone()) {
                Ok(_) => {
                    let ui = weak.unwrap();
                    ui.global::<ModpackLogic>().invoke_apply_modpack(mods);
                }
                Err(e) => eprintln!("{e}"),
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
                if let Ok(mut cfg) = R4D_CFG.try_lock() {
                    let ui = ui_apply_handle.upgrade().unwrap();
                    if vec_mods.is_empty() {
                        vec_mods = cfg
                            .priority
                            .clone()
                            .iter()
                            .map(|v| v.dir_name())
                            .flatten()
                            .collect();
                        cfg.applied_pack = "".to_string();
                    } else {
                        cfg.applied_pack = ui.get_active_pack().to_string();
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
                                    let mut buf = match get_modpacks_folder() {
                                        Ok(buf) => buf,
                                        Err(e) => {
                                            eprintln!("{e}");
                                            return;
                                        }
                                    };
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

    // Finish init of modpacks screen
    {
        let pack = match R4D_CFG.try_lock() {
            Ok(cfg) => cfg.applied_pack.clone(),
            Err(_) => "All Mods".to_owned(),
        };
        ui.global::<ModpackLogic>()
            .invoke_change_modpack(pack.into());
    }
}

pub async fn load_mod_packs() -> std::io::Result<HashMap<String, ModPack>> {
    let packs_dir = get_modpacks_folder();
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
        let lines = fs::read_to_string(entry.path()).await?;
        let pack: ModPack = sonic_rs::from_str(lines.as_str())?;
        packs.insert(pack.name.clone(), pack);
    }

    Ok(packs)
}

pub fn get_modpacks_folder() -> std::io::Result<PathBuf> {
    match get_config_dir() {
        Ok(mut config_dir) => {
            let diva_dir = match get_diva_folder() {
                Some(diva_dir) => diva_dir,
                None => "DefaultPath".to_owned(),
            };
            config_dir.push(filenamify(hash_dir_name(diva_dir.clone())));
            if !config_dir.try_exists()? {
                std::fs::create_dir(config_dir.clone())?
            }
            config_dir.push("modpacks");
            if !config_dir.try_exists()? {
                std::fs::create_dir(config_dir.clone())?
            }
            Ok(config_dir)
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn save_modpack(pack: ModPack) -> std::io::Result<()> {
    match get_modpacks_folder() {
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
pub fn save_modpack_sync(pack: ModPack) -> std::io::Result<()> {
    match get_modpacks_folder() {
        Ok(mut packs_dir) => {
            packs_dir.push(filenamify(pack.name.clone()) + ".json");
            if let Ok(pckstr) = sonic_rs::to_string_pretty(&pack.clone()) {
                return std::fs::write(packs_dir, pckstr);
            }
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn apply_mod_priority() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Ok(cfg) = R4D_CFG.try_lock() {
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
            prio = cfg
                .priority
                .clone()
                .iter()
                .map(|v| v.dir_name())
                .flatten()
                .collect();
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
