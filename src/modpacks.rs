use filenamify::filenamify;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use sonic_rs::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::vec;
use tokio::fs;
use tokio::sync::Mutex;

use crate::diva::{get_config_dir, get_diva_folder, open_error_window};
use crate::modmanagement::DivaMod;
use crate::slint_generatedApp::App;
use crate::{
    ConfirmDeletePack, DivaData, DivaModElement, ModPackElement, ModpackLogic, WindowLogic,
    DIVA_CFG, DML_CFG, MOD_PACKS,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModPack {
    name: String,
    mods: Vec<ModPackMod>,
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
}

impl ModPack {
    pub async fn to_element(self: &Self, diva: &Arc<Mutex<DivaData>>) -> ModPackElement {
        let diva = diva.lock().await;

        let vec_mod: VecModel<DivaModElement> = VecModel::from(vec![]);
        for module in self.mods.clone() {
            if let Some(module) = diva
                .mods
                .iter()
                .find(|&diva_mod| diva_mod.config.name == module.name)
            {
                vec_mod.push(module.to_element());
            }
        }

        return ModPackElement {
            name: self.name.clone().into(),
            mods: ModelRc::new(vec_mod),
            path: SharedString::from(""),
        };
    }

    pub fn new(name: String) -> Self {
        Self {
            name: name.clone(),
            mods: Vec::new(),
        }
    }
}

pub async fn init(ui: &App, _diva_arc: Arc<Mutex<DivaData>>) {
    let ui_add_mod_handle = ui.as_weak();
    let ui_remove_mod_handle = ui.as_weak();
    let ui_change_handle = ui.as_weak();
    let ui_add_pack_handle = ui.as_weak();
    let ui_save_handle = ui.as_weak();
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
                    for (pack, _mods) in gpacks.iter() {
                        ui_packs.push(pack.into());
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("{}", e);
        }
    }

    ui.global::<ModpackLogic>().on_add_mod_to_pack(
        move |diva_mod_element: DivaModElement, mod_pack| {
            let ui = ui_add_mod_handle.upgrade().unwrap();
            let pack_mods = ui.get_pack_mods();
            // ui.
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
        move |diva_mod_element: DivaModElement, mod_pack| {
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
                if let Ok(packs) = MOD_PACKS.lock() {
                    let pack = packs.get(&mod_pack.clone().to_string()).clone();
                    if pack.is_none() {
                        return;
                    }
                    let pack = pack.unwrap();
                    let pack = pack.clone();
                    ui_change_handle
                        .upgrade_in_event_loop(move |ui| {
                            let pack_mods: VecModel<DivaModElement> = VecModel::default();
                            for module in pack.mods.clone() {
                                pack_mods.push(module.to_element());
                            }
                            ui.set_pack_mods(ModelRc::new(pack_mods));
                        })
                        .expect("UI Update should have happened");
                }
            });
        });

    ui.global::<ModpackLogic>().on_create_new_pack(move |pack| {
        println!("We even get called @ modpack.rs 185");

        let pack_name = filenamify(pack.to_string());

        if pack_name.len() <= 0 {
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
                    println!("path: {}", m.path);
                    if let Some(dir) = m.dir_name() {
                        vec_mods.push(dir);
                    }
                }
                println!("{:?}", vec_mods);
                if vec_mods.is_empty() {
                    vec_mods = DIVA_CFG.lock().unwrap().priority.clone();
                }
                let ui_apply_handle = ui_apply_handle.clone();
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
                                            let _ = ui_apply_handle.clone().upgrade_in_event_loop(
                                                move |ui| {
                                                    let msg = format!(
                                                        "Unable to save modpack: \n{}",
                                                        e.to_string()
                                                    );
                                                    ui.invoke_open_error_dialog(msg.into());
                                                },
                                            );
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
                                    buf.push(format!("{}.json", pack.name));
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
                                            let _ = ui_delete_handle.clone().upgrade_in_event_loop(
                                                move |ui| {
                                                    let msg = format!(
                                                        "Unable to delete modpack: \n{}",
                                                        e.to_string()
                                                    );
                                                    ui.invoke_open_error_dialog(msg.into());
                                                },
                                            );
                                        }
                                    }
                                });
                            }
                            None => {}
                        }
                    }
                });
        });
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
            config_dir.push("modpacks");
            if !config_dir.exists() {
                fs::create_dir(config_dir.clone()).await?
            }
            Ok(config_dir)
        }
        Err(e) => Err(e.into()),
    }
}
