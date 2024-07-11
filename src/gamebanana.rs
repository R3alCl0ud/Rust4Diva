use std::env;
use std::fs::File;
use std::io::{BufRead, Write};

use curl::easy::Easy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sonic_rs::JsonValueTrait;

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
            let mod_data: sonic_rs::Value = sonic_rs::from_str(data_json.as_str()).unwrap();
            let mut dl_files: Vec<GbModDownload> = Vec::new();

            let info_mods = mod_data[1].clone().into_object().unwrap();
            // println!("{:?}", info_mods);
            for (key, value) in info_mods.iter() {
                // let file_str = sonic_rs::to_string(value).unwrap();
                let dl_file: GbModDownload = sonic_rs::from_value(value).unwrap();
                println!("{:?}: {:?}", key, dl_file);
                dl_files.push(dl_file);
            }
            the_mod = Some(GBMod {
                name: mod_data[0].to_string(),
                files: dl_files,
            });
            return Ok(data.len());
        }).expect("TODO: panic message");
        transfer.perform().unwrap();
    }
    easy.perform().unwrap();
    return the_mod;
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

pub fn download_mod_file(gb_file: &GbModDownload) -> std::io::Result<File> {
    println!("{}", gb_file._sFile);
    let mut dst = Vec::new();
    let mut easy = Easy::new();
    easy.url(gb_file._sDownloadUrl.as_str()).unwrap();
    let _redirect = easy.follow_location(true);

    {
        let mut transfer = easy.transfer();
        transfer.write_function(|data| {
            dst.extend_from_slice(data);
            // println!("File fetched");
            Ok(data.len())
        }).unwrap();
        println!("fetching file");
        transfer.perform().unwrap();
    }

    let mut file = File::create("/tmp/rust4diva/".to_owned() + &gb_file._sFile)?;
    println!("Writing file");
    file.write_all(dst.as_slice())?;
    println!("Done writing file to tmp directory");


    return Ok(file);
    // println!("{}", gb_file._sFile);
}