use std::io::ErrorKind;

use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::diva::get_config_dir;

#[derive(Deserialize, Serialize, Clone)]
pub struct DivaConfig {
    /// this is the global priority order, this is used when no modpack is applied
    pub priority: Vec<String>,
    pub diva_dir: String,
    // pub 
}

pub async fn load_diva_config() -> std::io::Result<DivaConfig> {
    let mut cfg_dir = get_config_dir().await?;
    cfg_dir.push("rust4diva.toml");
    if !cfg_dir.exists() {
        let cfg = DivaConfig {
            priority: Vec::new(),
            diva_dir:"".to_string()
        };
        return match toml::to_string(&cfg) {
            Ok(cfg_str) => {
                fs::write(cfg_dir, cfg_str).await?;
                Ok(cfg)
            }
            Err(e) => {
                Err(std::io::Error::new(ErrorKind::Other, e.to_string()))
            }
        };
    }
    Ok(DivaConfig {
        priority: Vec::new(),
        diva_dir: "".to_string()
    })
}


pub async fn write_config(cfg: DivaConfig) -> std::io::Result<()> {
    let mut cfg_dir = get_config_dir().await?;
    cfg_dir.push("rust4diva.toml");
    return match toml::to_string(&cfg) {
        Ok(cfg_str) => {
            fs::write(cfg_dir, cfg_str).await?;
            Ok(())
        }
        Err(e) => {
            Err(std::io::Error::new(ErrorKind::Other, e.to_string()))
        }
    };
}