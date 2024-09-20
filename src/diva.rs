use keyvalues_parser::Vdf;
use slint_interpreter::invoke_from_event_loop;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::{env, fs};

use crate::{ErrorMessageWindow, DIVA_CFG, DIVA_DIR, MAIN_UI_WEAK};
use slint::ComponentHandle;

cfg_if::cfg_if! {
    if #[cfg(windows)] {
        use winreg::enums::*;
        use winreg::RegKey;
    }
}

pub const STEAM_FOLDER: &str = ".local/share/Steam";
pub const STEAM_FOLDER_MAC: &str = "Library/Application Support/Steam";
pub const STEAM_LIBRARIES_CONFIG: &str = "config/libraryfolders.vdf";
pub const MEGA_MIX_APP_ID: &str = "1761390";
pub const DIVA_MOD_FOLDER_SUFFIX: &str = "steamapps/common/Hatsune Miku Project DIVA Mega Mix Plus";

pub fn create_tmp_if_not() -> std::io::Result<()> {
    match get_temp_folder() {
        Some(p) => {
            let path = Path::new(&p);
            if !path.exists() {
                let dir = fs::create_dir(path);
                return dir;
            }
            Ok(())
        }
        None => Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "Unknown temp dir path",
        )),
    }
}

pub fn get_temp_folder() -> Option<String> {
    match env::consts::OS {
        "linux" | "macos" => Some("/tmp/rust4diva".to_string()),
        "windows" => {
            let mut tmp = dirs::data_local_dir().unwrap();
            tmp.push("Temp");
            let temp = tmp.as_os_str();
            match temp.to_str() {
                Some(s) => {
                    let t = s.to_owned();
                    Some(t)
                }
                None => None,
            }
        }
        os => {
            println!("Unknown OS: {}", os);
            None
        }
    }
}

pub fn get_steam_folder() -> Option<String> {
    if let Ok(cfg) = DIVA_CFG.try_lock() {
        if !cfg.steam_dir.is_empty() && PathBuf::from(cfg.steam_dir.clone()).exists() {
            return Some(cfg.steam_dir.clone());
        }
    }
    println!("Attempting to find the Steam folder");
    return match env::consts::OS {
        "linux" => {
            let mut binding = dirs::home_dir().unwrap();
            binding.push(STEAM_FOLDER);
            if !binding.exists() {
                println!("Regular Steam folder not found, searching for flatpak isntead");
                binding = dirs::home_dir().unwrap();
                binding.push(".var/app/com.valvesoftware.Steam/data/Steam");
                if !binding.exists() {
                    println!("Can't find flatpak steam.");
                    return None;
                }
            }
            Some(binding.display().to_string())
        }
        "macos" => {
            let mut binding = dirs::home_dir().unwrap();
            binding.push(STEAM_FOLDER_MAC);
            if !binding.exists() {
                println!("Steam folder not found");
            }
            Some(binding.display().to_string())
        }
        "windows" => {
            // only compiles on windows
            cfg_if::cfg_if! {
                if #[cfg(windows)] {
                    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
                    let steam_key = hklm.open_subkey(r#"SOFTWARE\WOW6432Node\Valve\Steam"#);
                    if steam_key.is_err() {
                        return None;
                    }
                    let steam_key = steam_key.unwrap();
                    let res: std::io::Result<String> = steam_key.get_value("InstallPath");
                    if let Ok(path) = res {
                        println!("{}", path);
                        if PathBuf::from(path.clone()).exists() {
                            return Some(path.clone());
                        } else {
                            return Some(r#"C:\Program Files (x86)\Steam"#.to_string());
                        }
                    } else {
                        return Some(r#"C:\Program Files (x86)\Steam"#.to_string());
                    }
                } else {
                   None
                }
            }
        }
        os => {
            println!("Unsupported Operating system: {}", os);
            None
        }
    };
}

pub fn get_diva_folder() -> Option<String> {
    if let Ok(dir) = DIVA_DIR.try_lock() {
        return Some(dir.clone());
    }
    return find_diva_folder();
}

pub fn find_diva_folder() -> Option<String> {
    // try retreiving from the config second
    if let Ok(cfg) = DIVA_CFG.try_lock() {
        let mut buf = PathBuf::from(cfg.diva_dir.clone());
        if !cfg.diva_dir.is_empty() && buf.exists() {
            buf.push("DivaMegaMix.exe");
            if buf.exists() {
                buf.pop();
                return Some(cfg.diva_dir.clone());
            }
        }
    } else {
        println!("couldn't lock, Looking for Project Diva folder");
    }

    match get_steam_folder() {
        Some(steam_folder) => {
            let mut path = "".to_owned();
            let mut lib_path = PathBuf::new();
            lib_path.push(steam_folder);
            lib_path.push(STEAM_LIBRARIES_CONFIG);
            if !lib_path.exists() {
                return None;
            }

            let binding = fs::read_to_string(lib_path).unwrap();
            let lf_res = Vdf::parse(binding.as_str());
            match lf_res {
                Ok(libraryfolders) => {
                    let libraries = libraryfolders.value.unwrap_obj();
                    for library_id in libraries.clone().keys() {
                        // get the library obj
                        let library = libraries
                            .get(library_id)
                            .unwrap()
                            .first()
                            .unwrap()
                            .clone()
                            .unwrap_obj();
                        // prevent a crash in case of malformed libraryfolders.vdf
                        if !library.contains_key("apps") || !library.contains_key("path") {
                            continue;
                        }
                        // get the list of apps installed to this library
                        let apps = library
                            .get("apps")
                            .unwrap()
                            .first()
                            .unwrap()
                            .clone()
                            .unwrap_obj();
                        // self-explanatory
                        if apps.contains_key(MEGA_MIX_APP_ID) {
                            // get the path of the library
                            let path_str =
                                library.get("path").unwrap().first().unwrap().to_string();
                            // this is set up for removing the quotes
                            let mut path_chars = path_str.chars();
                            // remove the quotes from the value
                            path_chars.next();
                            path_chars.next_back();
                            // concat the strings together properly
                            let mut buf = PathBuf::new();
                            buf.push(path_chars.as_str());
                            let diva = PathBuf::from(DIVA_MOD_FOLDER_SUFFIX);
                            buf.push(diva.as_os_str());
                            path = buf
                                .canonicalize()
                                .unwrap()
                                .as_os_str()
                                .to_str()
                                .unwrap()
                                .to_string();
                            println!("PD MMP Folder: {:?}", path);
                            break;
                        }
                    }
                    Some(path)
                }
                Err(e) => {
                    eprintln!("{}", e);
                    None
                }
            }
        }
        None => {
            return None;
        }
    }
}

pub async fn get_config_dir() -> std::io::Result<PathBuf> {
    match dirs::config_dir() {
        Some(mut buf) => {
            buf.push("rust4diva");
            if !buf.exists() {
                fs::create_dir(buf.clone())?;
            }
            Ok(buf.clone())
        }
        None => Err(Error::new(
            ErrorKind::NotFound,
            "Unable to get config directory",
        )),
    }
}

pub fn get_config_dir_sync() -> std::io::Result<PathBuf> {
    match dirs::config_dir() {
        Some(mut buf) => {
            buf.push("rust4diva");
            if !buf.exists() {
                fs::create_dir(buf.clone())?;
            }
            Ok(buf.clone())
        }
        None => Err(Error::new(
            ErrorKind::NotFound,
            "Unable to get config directory",
        )),
    }
}

pub static MIKU_ART: &'static str = r#"
　　🟦　　　　　　　　　　　　　　🟦　　　　　　　　　🟦🟦🟦　　　　　　　　　　　🟦　　　　
　　🟦　　🟦🟦🟦🟦🟦🟦　　🟦🟦🟦🟦🟦🟦🟦🟦🟦🟦　　　　　　🟦🟦🟦🟦　　　　　　　🟦🟦🟦🟦🟦🟦
　🟦🟦🟦🟦　　🟦　　🟦　　　　🟦　　　　🟦　　　　　　　　　　　　　　　　　　🟦🟦　　　　🟦
　　　　🟦　　🟦　　🟦　　　　🟦　　　　🟦　　　　　　　　　　　　　　　　　🟦🟦　　　　　🟦
　　　🟦　　　🟦　　🟦　🟦🟦🟦🟦🟦🟦🟦🟦🟦🟦🟦🟦　　　🟦🟦🟦🟦🟦　　　　　　🟦　　　　　🟦🟦
　　🟦🟦🟦　　🟦　　🟦　　　🟦🟦🟦🟦🟦🟦🟦🟦　　　　　　　　　　🟦　　　　　　　　　　　🟦　
🟦🟦🟦🟦🟦　🟦🟦　　🟦　　　🟦　　　　　　🟦　　　　　　　　　　　　　　　　　　　　　🟦🟦　
　　🟦　🟦　🟦　　　🟦　　　🟦🟦🟦🟦🟦🟦🟦🟦　　　　🟦🟦🟦　　　　　　　　　　　　　　🟦　　
　　🟦　　　🟦　　　🟦　　　🟦　　　　　　🟦　　　　　　🟦🟦🟦🟦🟦　　　　　　　　　🟦　　　
　　🟦　　🟦　　　　🟦　　　🟦🟦🟦🟦🟦🟦🟦🟦　　　　　　　　　　🟦🟦　　　　　　🟦🟦　　　　
　　🟦　🟦　　　🟦🟦　　　　🟦　　　　　　🟦　　　　　　　　　　　　　　　　　🟦🟦　　　　　"#;

pub fn open_error_window(message: String) {
    println!("{message}");
    let _ = invoke_from_event_loop(move || match ErrorMessageWindow::new() {
        Ok(error_win) => {
            if let Ok(opt) = MAIN_UI_WEAK.try_lock() {
                if let Some(main_ui_weak) = opt.clone() {
                    if let Some(main_ui) = main_ui_weak.upgrade() {
                        let error_weak = error_win.as_weak();
                        main_ui.on_close_windows(move || {
                            error_weak.unwrap().hide().unwrap();
                        });
                    }
                    error_win.set_msg(message.into());
                    let close_handle = error_win.as_weak();
                    error_win.on_close(move || {
                        close_handle.upgrade().unwrap().hide().unwrap();
                    });
                    error_win.show().unwrap();
                }
            }
        }
        Err(e) => {
            eprintln!("{e}");
        }
    });
}
