use std::alloc::handle_alloc_error;
use std::env;
use std::io::{BufRead};
use std::ptr::null;
use curl::easy::Easy;
use regex::Regex;
use sonic_rs::{JsonValueTrait};
use serde::{Deserialize, Serialize};

const GB_API_DOMAIN: &str = "https://api.gamebanana.com";
const GB_DOMAIN: &str = "https://gamebanana.com";
const GB_DIVA_ID: i32 = 16522;

const GB_MOD_INFO: &str = "/Core/Item/Data";


#[derive(Serialize, Deserialize, Clone)]
pub struct GbModDownload {
    _idRow: u32,
    _sFile: String,
    _nFilesize: u32,
    _sDescription: String,
    _tsDateAdded: u32,
    _nDownloadCount: u32,
    _sMd5Checksum: String,
    _sDownloadUrl: String,
    _sClamAvResult: String,
    _sAvastAvResult: String,
    _sAnalysisState: String,
    _sAnalysisResult: String,
    _sAnalysisResultCode: String,
    _bContainsExe: bool,
}

pub fn fetch_mod_data(mod_id: &str) -> Vec<GbModDownload> {
    // let stream = InMemoryStream::default();
    println!("Fetching Mod data for: {}", mod_id);
    let mut mods: Vec<GbModDownload> = Vec::new();
    let mut easy = Easy::new();
    easy.url(format!("{}{}?itemid={}&itemtype=Mod&fields=name,Files().aFiles()", GB_API_DOMAIN, GB_MOD_INFO, mod_id).as_str()).unwrap();
    {
        let mut transfer = easy.transfer();
        transfer.write_function(|data| {
            let mut data_json: String = String::new();
            for line in data.lines() {
                data_json.push_str(line.unwrap().as_str());
            }
            let mod_data: sonic_rs::Value = sonic_rs::from_str(data_json.as_str()).unwrap();
            let str_data = sonic_rs::to_string(&mod_data[1][get_diva_dl_id().as_str()]).unwrap();
            if !str_data.is_empty() {
                let mod_info: GbModDownload = sonic_rs::from_str(str_data.as_str()).unwrap();
                println!("test {:#?}", mod_info._sFile);
                mods.push(mod_info);
            }
            Ok(data.len())
        });
        transfer.perform().unwrap();
    }
    // easy.write_function(move |data| {
    //     let mut data_json: String = String::new();
    //     // stdout().write_all(data).unwrap();
    //     for line in data.lines() {
    //         data_json.push_str(line.unwrap().as_str());
    //     }
    //     let mod_data: sonic_rs::Value = sonic_rs::from_str(data_json.as_str()).unwrap();
    //     let mod_name = mod_data[0].as_str().unwrap();
    //     let str_data = sonic_rs::to_string(&mod_data[1][get_diva_dl_id().as_str()]).unwrap();
    //     if !str_data.is_empty() {
    //         let mod_info: GbModDownload = sonic_rs::from_str(str_data.as_str()).unwrap();
    //         println!("test {:#?}", mod_info._sFile);
    //         mods.push(mod_info);
    //         let mut dl_mod_archive= Easy::new();
    //         // dl_mod_archive.url(mod_info._sDownloadUrl.as_str());
    //
    //     }
    //     Ok(data.len())
    // }).unwrap();
    easy.perform().unwrap();
    return mods;
}


pub fn get_diva_dl_id() -> String {
    let mod_regex = Regex::new(r"([0-9]+),(.+),([0-9]+)").unwrap();
    let args = env::args().collect::<Vec<String>>();
    let Some(m_info) = mod_regex.captures(args.get(1).unwrap()) else {
        println!("Sorry, no fucks in here");
        return String::new();
    };
    // println!("Mod DL ID: {:?} \nMod ID: {:?}", m_info.get(1).unwrap().as_str(), m_info.get(3).unwrap().as_str());

    return format!("{}", m_info.get(1).unwrap().as_str());
}