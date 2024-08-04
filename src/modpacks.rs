use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use std::sync::Arc;

use sonic_rs::{Deserialize, Serialize};
use tokio::fs;
use tokio::sync::Mutex;

use crate::{App, DivaData};
use crate::diva::get_config_dir;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModPack {
    name: String,
    mods: Vec<String>,
}


pub async fn init(ui: &App, diva_arc: Arc<Mutex<DivaData>>) {}

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




