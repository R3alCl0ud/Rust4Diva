use std::fs::File;
use std::io::{BufRead, Write};

use curl::easy::Easy;
use futures_util::StreamExt;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sonic_rs::Error;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::DlFinish;

const GB_API_DOMAIN: &str = "https://api.gamebanana.com";
const _GB_DOMAIN: &str = "https://gamebanana.com";
const _GB_DIVA_ID: i32 = 16522;

const GB_MOD_INFO: &str = "/Core/Item/Data";


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GbModDownload {
    pub(crate) _idRow: u32,
    pub(crate) _sFile: String,
    pub(crate) _nFilesize: u32,
    pub(crate) _sDescription: String,
    pub(crate) _tsDateAdded: u32,
    pub(crate) _nDownloadCount: u32,
    pub(crate) _sMd5Checksum: String,
    pub(crate) _sDownloadUrl: String,
    pub(crate) _sClamAvResult: String,
    pub(crate) _sAvastAvResult: String,
    pub(crate) _sAnalysisState: String,
    pub(crate) _sAnalysisResult: String,
    pub(crate) _sAnalysisResultCode: String,
    pub(crate) _bContainsExe: bool,
}

#[derive(Clone, Debug)]
pub struct GBMod {
    pub(crate) name: String,
    pub(crate) files: Vec<GbModDownload>,
    pub(crate) text: String,
}
pub fn fetch_mod_data(mod_id: &str) -> Option<GBMod> {
    // let stream = InMemoryStream::default();
    if mod_id.is_empty() {
        return None;
    }
    println!("Fetching Mod data for: {}", &mod_id);

    // let mut mods: Vec<GBMod> = Vec::new();
    let mut easy = Easy::new();
    let mut the_mod = None;
    easy.url(format!("{}{}?itemid={}&itemtype=Mod&fields=name,Files().aFiles()", GB_API_DOMAIN, GB_MOD_INFO, &mod_id).as_str()).unwrap();
    {
        let mut transfer = easy.transfer();
        transfer.write_function(|data| {
            let mut data_json: String = String::new();
            for line in data.lines() {
                data_json.push_str(line.unwrap().as_str());
            }
            // println!("{:#}", data_json);
            let mut res: Result<sonic_rs::Value, Error> = sonic_rs::from_str(data_json.as_str());
            let mut dl_files: Vec<GbModDownload> = Vec::new();
            if res.is_ok() {
                let mod_data = res.unwrap();
                let info_mods = mod_data[1].clone().into_object();
                // make sure we've actually got a proper response data
                if info_mods.is_some() {
                    for (key, value) in info_mods.unwrap().iter() {
                        let dl_file: GbModDownload = sonic_rs::from_value(value).unwrap();
                        // println!("{:?}: {:?}", key, dl_file);
                        dl_files.push(dl_file);
                    }
                    the_mod = Some(GBMod {
                        name: mod_data[0].to_string(),
                        files: dl_files,
                        text: mod_data[2].to_string(),
                    });
                }
            }

            return Ok(data.len());
        }).expect("TODO: panic message");
        transfer.perform().unwrap();
    }
    easy.perform().unwrap();
    return the_mod;
}

pub struct GbDmmItem {
    pub(crate) item_id: String,
    pub(crate) itemtype: String,
    pub(crate) file_id: String,
}

pub fn parse_dmm_url(dmm_url: String) -> Option<GbDmmItem> {
    // check if this is a proper dmm 1 click url
    if !dmm_url.starts_with("divamodmanager:https://gamebanana.com/mmdl/") {
        return None;
    }

    let mod_regex = Regex::new(r"([0-9]+),(.+),([0-9]+)").unwrap();
    let Some(m_info) = mod_regex.captures(dmm_url.as_str()) else {
        println!("Sorry, no fucks in here");
        return None;
    };
    return Some(GbDmmItem {
        item_id: m_info.get(0).unwrap().as_str().to_string(),
        itemtype: m_info.get(1).unwrap().as_str().to_string(),
        file_id: m_info.get(2).unwrap().as_str().to_string(),
    });
}

pub fn download_mod_file(gb_file: &GbModDownload, sender: Sender<DlFinish>) -> Receiver<u64> {
    println!("{}", gb_file._sFile);
    let (tx, rx) = channel::<u64>(2048);
    let gb_file = gb_file.clone();
    tokio::spawn(async move {
        let mut dst = Vec::new();
        let mut easy = Easy::new();
        easy.url(gb_file._sDownloadUrl.as_str()).unwrap();
        let _redirect = easy.follow_location(true);
        let mut downloaded_size: u64 = 0;
        {
            let mut transfer = easy.transfer();
            transfer.write_function(|data| {
                dst.extend_from_slice(data);
                downloaded_size = downloaded_size + data.len() as u64;
                let _ = tx.try_send(downloaded_size);
                Ok(data.len())
            }).unwrap();
            println!("fetching file");
            transfer.perform().unwrap();
        }
        let mut file_res = File::create("/tmp/rust4diva/".to_owned() + &gb_file._sFile);
        match file_res {
            Ok(mut file) => {
                println!("Writing file");
                file.write_all(dst.as_slice()).expect("Failed to write to file");
                sender.send(DlFinish {
                    file: gb_file,
                    success: true,
                }).await.unwrap();
            }
            Err(e) => {
                sender.send(DlFinish {
                    file: gb_file,
                    success: false,
                }).await.expect("uh oh");
            }
        }
    });
    return rx;
}

pub fn reqwest_mod_data(gb_file: &GbModDownload, sender: Sender<Option<GbModDownload>>) -> tokio::sync::mpsc::Receiver<u64> {
    let (tx, rx) = channel::<u64>(2048);
    let gb_file = gb_file.clone();
    tokio::spawn(async move {
        println!("Begin retrieving the mod file");
        let gb_dl = gb_file.clone();

        let res = reqwest::get(&gb_dl._sDownloadUrl).await;
        match res {
            Ok(res) => {
                println!("Has received response from GameBanana");
                let mut stream = &mut res.bytes_stream();
                let mut handle = File::create("/tmp/rust4diva/".to_owned() + &*gb_dl._sFile);
                match handle {
                    Ok(mut file) => {
                        let mut all_good = true;
                        let mut downloaded_size: u64 = 0;
                        while let Some(data) = stream.next().await {
                            let chunk_res = data.or(Err("Error While Downloading file"));
                            match chunk_res {
                                Ok(chunk) => {
                                    downloaded_size = downloaded_size + (chunk.len() as u64);
                                    match file.write_all(&chunk) {
                                        Ok(..) => {
                                            let _ = tx.try_send(downloaded_size);
                                            // match  {
                                            // Ok(_) => {
                                            //     println!("Progress sent");
                                            // }
                                            // Err(e) => {
                                            //     eprintln!("Failed to send progress back to ui thread: {}", e);
                                            // }
                                            // }
                                            // tx.send(downloaded_size)
                                        }
                                        Err(e) => {
                                            eprintln!("Fuck: {}", e);
                                            all_good = false;
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    all_good = false;
                                    eprintln!("Download ended early: {}", e);
                                }
                            }
                        }
                        if downloaded_size < gb_file._nFilesize as u64 {
                            all_good = false;
                        }
                        if all_good {
                            println!("Saving file to disk: {}", &gb_dl._sFile);
                            if let Err(e) = sender.send(Some(gb_dl)).await {
                                eprintln!("Unable to communicate with UI Thread: {}", e);
                            }
                        } else {
                            eprintln!("Something went wrong during the download");
                            sender.send(None).await.expect("Fuck");
                        }
                    }
                    Err(w) => {
                        eprintln!("{}", w);
                        sender.send(None).await.expect("Fuck");
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed: {:#?}", e);
                sender.send(None).await.expect("Fuck");
            }
        }
    });
    return rx;
}


pub fn _download_mod_file_from_id(gb_file_id: String) -> std::io::Result<File> {
    println!("{}", gb_file_id);
    let mut dst = Vec::new();
    let mut easy = Easy::new();
    easy.url(gb_file_id.as_str()).unwrap();
    let _redirect = easy.follow_location(true);

    {
        let mut transfer = easy.transfer();
        transfer.write_function(|data| {
            dst.extend_from_slice(data);
            Ok(data.len())
        }).unwrap();
        println!("fetching file");
        transfer.perform().unwrap();
    }

    let mut file = File::create("/tmp/rust4diva/".to_owned() + &gb_file_id)?;
    println!("Writing file");
    file.write_all(dst.as_slice())?;
    println!("Done writing file to tmp directory");


    return Ok(file);
}
