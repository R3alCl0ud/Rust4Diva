use std::env;
use std::fs::File;
use std::io::{BufRead, Write};

use curl::easy::Easy;
use regex::Regex;
use reqwest::header;
use serde::{Deserialize, Serialize};
use sonic_rs::{Error, JsonValueTrait};
use crate::modmanagement::create_tmp_if_not;

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
            println!("{:#}", data_json);
            let mut res: Result<sonic_rs::Value, Error> = sonic_rs::from_str(data_json.as_str());
            let mut dl_files: Vec<GbModDownload> = Vec::new();
            if res.is_ok() {
                let mod_data = res.unwrap();
                let info_mods = mod_data[1].clone().into_object();
                // match info_mods {
                // make sure we've actually got a proper response data
                if info_mods.is_some() {
                    for (key, value) in info_mods.unwrap().iter() {
                        // let file_str = sonic_rs::to_string(value).unwrap();
                        let dl_file: GbModDownload = sonic_rs::from_value(value).unwrap();
                        println!("{:?}: {:?}", key, dl_file);
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

struct GbUrlData {
    name: String,
    itemtype: String,
    file: String,
}

pub fn parse_dmm_url(dmm_url: String) -> Option<GbUrlData> {
    // check if this is a proper dmm 1 click url
    if !dmm_url.starts_with("divamodmanager:https://gamebanana.com/mmdl/") {
        return None;
    }

    let mod_regex = Regex::new(r"([0-9]+),(.+),([0-9]+)").unwrap();
    // let args = env::args().collect::<Vec<String>>();
    let Some(m_info) = mod_regex.captures(dmm_url.as_str()) else {
        println!("Sorry, no fucks in here");
        return None;
    };
    return Some(GbUrlData {
        name: m_info.get(0).unwrap().as_str().to_string(),
        itemtype: m_info.get(1).unwrap().as_str().to_string(),
        file: m_info.get(2).unwrap().as_str().to_string(),
    });
}

pub fn download_mod_file(gb_file: &GbModDownload) -> std::io::Result<()> {
    println!("{}", gb_file._sFile);
    let mut dst = Vec::new();
    let mut easy = Easy::new();
    easy.url(gb_file._sDownloadUrl.as_str()).unwrap();
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

    let mut file = File::create("/tmp/rust4diva/".to_owned() + &gb_file._sFile)?;
    let dir = create_tmp_if_not();
    match &dir {
        Ok(..) => {
            println!("Writing file");
            file.write_all(dst.as_slice())?;
            println!("Done writing file to tmp directory");
            return Ok(());
        }
        Err(e) => {
            return dir;
        }
    }
}

pub async fn reqwest_mod_data(gb_file: &GbModDownload) {
    let result = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            println!("Begin retrieving the mod file");
            let mut headers = header::HeaderMap::new();

            let client = reqwest::Client::builder()
                .default_headers(headers)
                .build()
                .unwrap();
            let response = client.get(&gb_file._sDownloadUrl).send();
            let t = response.await;
            match t {
                Ok(res) => {
                    println!("Saving file to disk: {}", &gb_file._sFile);
                    tokio::fs::write("/tmp/rust4diva/".to_owned() + &gb_file._sFile, res.bytes().await.unwrap()).await.expect("Fuck");
                    // return Ok::<_, Box<dyn std::error::Error>>(res.bytes());
                }
                Err(e) => {
                    eprintln!("Failed: {:#?}", e);
                }
            }
        });

    result
}


pub fn download_mod_file_from_id(gb_file_id: String) -> std::io::Result<File> {
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
