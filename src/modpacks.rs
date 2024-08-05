use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use sonic_rs::{Deserialize, Serialize};
use tokio::fs;
use tokio::sync::Mutex;

use crate::slint_generatedApp::App;
use crate::{DivaData, DivaModElement, ModPackElement};
use crate::diva::get_config_dir;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModPack {
    name: String,
    mods: Vec<String>,
}

impl ModPack {
    pub fn to_element(self: &Self) -> ModPackElement {
        let vec_mod: VecModel<SharedString> = VecModel::from(vec![]);
        for module in self.mods.clone() {
            vec_mod.push(module.into());
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
    let ui_reload_handle = ui.as_weak();
    let reload_diva = diva_arc.clone();


    let diva = init_diva.lock().await;


    ui.on_add_mod_to_pack(move |diva_mod_element: DivaModElement| {
        println!("{}", diva_mod_element.name);
        let ui = ui_add_mod_handle.upgrade().unwrap();
        let mut pack_mods = ui.get_pack_mods();
        if let Some(pack_mods) = pack_mods.as_any().downcast_ref::<VecModel<DivaModElement>>() {
            for element in pack_mods.iter() {
                if element.name == diva_mod_element.name {
                    println!("Mod already in pack");
                    return;
                }
            }
            println!("Pushing mod @ modpacks.rs 55");
            pack_mods.push(diva_mod_element);
        }
    });

    ui.on_remove_mod_from_pack(move |diva_mod_element: DivaModElement| {
        println!("{}", diva_mod_element.name);
        let ui = ui_remove_mod_handle.upgrade().unwrap();
        let mut pack_mods = ui.get_pack_mods();
        if let Some(pack_mods) = pack_mods.as_any().downcast_ref::<VecModel<DivaModElement>>() {
            let mut idx: usize = 0;
            for element in pack_mods.iter() {
                if element.name == diva_mod_element.name {
                    return;
                }
                println!("{:?}", idx += 1);
            }
        }
    });
}

pub async fn load_mod_packs() -> std::io::Result<HashMap<String, ModPack>> {
    let packs_dir = get_modpacks_folder().await;
    if packs_dir.is_none() {
        return Err(Error::new(ErrorKind::NotFound, "Could not find modpacks folder"));
    }
    let mut packs: HashMap<String, ModPack> = HashMap::new();
    let packs_dir = packs_dir.unwrap();
    while let Ok(entry) = fs::read_dir(packs_dir.clone()).await {
        println!("{:?}", entry);
    }


    Ok(packs)
}

pub async fn get_modpacks_folder() -> Option<String> {
    let config_dir = get_config_dir().await;

    None
}




