use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use sonic_rs::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;

use crate::diva::{get_config_dir, get_diva_folder};
use crate::modmanagement::DivaMod;
use crate::slint_generatedApp::App;
use crate::{DivaData, DivaModElement, ModPackElement};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModPack {
    name: String,
    mods: Vec<ModPackMod>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModPackMod {
    pub name: String,
    pub enabled: bool,
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
            description:SharedString::from(""),
            version: SharedString::from(""),
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
        };
    }
}

pub async fn init(ui: &App, diva_arc: Arc<Mutex<DivaData>>) {
    let init_diva = diva_arc.clone();
    let ui_add_mod_handle = ui.as_weak();
    let ui_remove_mod_handle = ui.as_weak();
    let ui_change_handle = ui.as_weak();
    // let ui_save_handle = ui.as_weak();
    let change_diva = diva_arc.clone();
    let apply_diva = diva_arc.clone();
    let save_diva = diva_arc.clone();

    let mut diva = init_diva.lock().await;

    match load_mod_packs().await {
        Ok(mods) => {
            diva.mod_packs = mods;
            let binding = ui.get_modpacks();
            match binding.as_any().downcast_ref::<VecModel<SharedString>>() {
                None => {}
                Some(ui_packs) => {
                    for (pack, mods) in diva.mod_packs.iter() {
                        ui_packs.push(pack.into());
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("{}", e);
        }
    }

    ui.on_add_mod_to_pack(move |diva_mod_element: DivaModElement, mod_pack| {
        let ui = ui_add_mod_handle.upgrade().unwrap();
        let mut pack_mods = ui.get_pack_mods();
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
    });

    ui.on_remove_mod_from_pack(move |diva_mod_element: DivaModElement, mod_pack| {
        let ui = ui_remove_mod_handle.upgrade().unwrap();
        let mut pack_mods = ui.get_pack_mods();
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
    });

    ui.on_modpack_changed(move |mod_pack| {
        println!("{}", mod_pack);
        let ui_change_handle = ui_change_handle.clone();
        let change_diva = change_diva.clone();
        tokio::spawn(async move {
            let diva = change_diva.lock().await;
            let packs = diva.mod_packs.clone();
            let pack = packs.get(&mod_pack.clone().to_string()).clone();
            if pack.is_none() {
                return;
            }
            let pack = pack.unwrap();
            println!("{}", pack.name.clone());
            let pack = pack.clone();
            ui_change_handle
                .upgrade_in_event_loop(move |ui| {
                    let pack_mods: VecModel<DivaModElement> = VecModel::default();
                    for module in pack.mods.clone() {
                        pack_mods.push(module.to_element());
                    }
                    ui.set_pack_mods(ModelRc::new(pack_mods));
                })
                .expect("test");
        });
    });

    ui.on_save_modpack(move |modpack, mods| {
        let modpack = modpack.to_string();
        let modpack = modpack.clone();
        let save_diva = save_diva.clone();
        match mods.as_any().downcast_ref::<VecModel<DivaModElement>>() {
            Some(mods) => {
                let mut vec_mods: Vec<DivaModElement> = Vec::new();
                for m in mods.iter() {
                    vec_mods.push(m.clone());
                }
                tokio::spawn(async move {
                    let mods = vec_mods.clone();
                    let mut diva = save_diva.lock().await;
                    if let Some(mut pack) = diva.mod_packs.get(&modpack) {
                        let mut pack = pack.clone();
                        let mut new_mods = vec![];
                        for m in mods.iter() {
                            new_mods.push(m.to_packmod());
                        }
                        pack.mods = new_mods;
                        diva.mod_packs.insert(modpack.clone(), pack.clone());
                        match sonic_rs::to_string_pretty(&pack) {
                            Ok(pack_str) => {
                                if let Ok(mut buf) = get_modpacks_folder().await {
                                    buf.push(format!("{modpack}.json"));
                                    match fs::write(buf, pack_str).await {
                                        Ok(..) => {}
                                        Err(e) => {
                                            eprintln!("{e}");
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("{}", e);
                            }
                        }
                    }
                });
            }
            None => {
                return;
            }
        }
    });

    ui.on_apply_modpack(move |mods| {
        let apply_diva = apply_diva.clone();
        match mods.as_any().downcast_ref::<VecModel<DivaModElement>>() {
            Some(mods) => {
                let mut vec_mods: Vec<String> = Vec::new();
                for m in mods.iter() {
                    
                    if m.enabled {
                        vec_mods.push(m.name.to_string());
                    }
                }
                println!("{:?}", vec_mods);
                tokio::spawn(async move {
                    let mut diva = apply_diva.lock().await;
                    if diva.dml.is_some() {
                        let mut dml = diva.dml.as_mut().unwrap();
                        dml.priority = vec_mods;
                        if let Ok(dmlcfg) = toml::to_string(&dml) {
                            if let Some(dir) = get_diva_folder() {
                                let mut buf = PathBuf::from(dir);
                                buf.push("config.toml");
                                if buf.exists() {
                                    match fs::write(buf, dmlcfg).await {
                                        Ok(_) => {
                                            println!("Mod pack successfully applied");
                                        },
                                        Err(_) => {},
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
