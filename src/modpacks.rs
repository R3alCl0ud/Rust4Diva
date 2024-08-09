use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use sonic_rs::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;

use crate::diva::get_config_dir;
use crate::slint_generatedApp::App;
use crate::{DivaData, DivaModElement, ModPackElement};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModPack {
    name: String,
    mods: Vec<ModPackMod>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModPackMod {
    name: String,
    enabled: bool,
}

impl ModPackMod {
    pub fn to_element(self: &Self) -> DivaModElement {
        DivaModElement {
            author: Default::default(),
            name: self.name.clone().into(),
            enabled: self.enabled,
            description: Default::default(),
            version: Default::default(),
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
    let ui_reload_handle = ui.as_weak();
    let change_diva = diva_arc.clone();
    let reload_diva = diva_arc.clone();

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
            let pack = packs.get(&mod_pack.to_string());
            if pack.is_none() {
                return;
            }
            let pack = pack.unwrap();
            println!("{}", pack.name.clone());
            ui_change_handle.upgrade_in_event_loop(|ui| {
                let pack_mods = ui.get_pack_mods();
                match pack_mods.as_any().downcast_ref::<VecModel<DivaModElement>>() {
                    Some(e) =>{}
                    None => {}
                }
            });
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
        // println!("{}", fuck);
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
