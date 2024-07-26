use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, Write};
use std::rc::Rc;
use std::sync::{Arc, MutexGuard};

use curl::easy::Easy;
use futures_util::StreamExt;
use regex::Regex;
use serde::{Deserialize, Serialize};
use slint::{ComponentHandle, Model, ModelRc, SharedString, StandardListViewItem, VecModel, Weak};
use sonic_rs::{Error, JsonContainerTrait, Value};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::Mutex;

use crate::{App, DivaData, DlFinish, Download};

const GB_API_DOMAIN: &str = "https://api.gamebanana.com";
const GB_DOMAIN: &str = "https://gamebanana.com";
const GB_DIVA_ID: i32 = 16522;

const GB_MOD_INFO: &str = "/Core/Item/Data";
const GB_MOD_SEARCH: &str = "apiv9/Util/Game/Submissions";


const DOWNLOAD_QUEUE: std::sync::Mutex<VecDeque<(i32, Download)>> = std::sync::Mutex::new(VecDeque::new());


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GbModDownload {
    #[serde(rename(serialize = "_idRow", deserialize = "_idRow"))]
    pub(crate) id: u32,
    #[serde(rename(serialize = "_sFile", deserialize = "_sFile"))]
    pub(crate) file: String,
    #[serde(rename(serialize = "_nFilesize", deserialize = "_nFilesize"))]
    pub(crate) filesize: u32,
    #[serde(rename(serialize = "_sDescription", deserialize = "_sDescription"))]
    pub(crate) description: String,
    #[serde(rename(serialize = "_tsDateAdded", deserialize = "_tsDateAdded"))]
    pub(crate) date_added: u32,
    #[serde(rename(serialize = "_nDownloadCount", deserialize = "_nDownloadCount"))]
    pub(crate) download_count: u32,
    #[serde(rename(serialize = "_sMd5Checksum", deserialize = "_sMd5Checksum"))]
    pub(crate) md5_checksum: String,
    #[serde(rename(serialize = "_sDownloadUrl", deserialize = "_sDownloadUrl"))]
    pub(crate) download_url: String,
    #[serde(rename(serialize = "_sClamAvResult", deserialize = "_sClamAvResult"))]
    pub(crate) clam_av_result: String,
    #[serde(rename(serialize = "_sAvastAvResult", deserialize = "_sAvastAvResult"))]
    pub(crate) avast_av_result: String,
    #[serde(rename(serialize = "_sAnalysisState", deserialize = "_sAnalysisState"))]
    pub(crate) analysis_state: String,
    #[serde(rename(serialize = "_sAnalysisResult", deserialize = "_sAnalysisResult"))]
    pub(crate) analysis_result: String,
    #[serde(rename(serialize = "_sAnalysisResultCode", deserialize = "_sAnalysisResultCode"))]
    pub(crate) analysis_result_code: String,
    #[serde(rename(serialize = "_bContainsExe", deserialize = "_bContainsExe"))]
    pub(crate) contains_exe: bool,

}

#[derive(Clone, Debug)]
pub struct GBMod {
    pub(crate) name: String,
    pub(crate) files: Vec<GbModDownload>,
    pub(crate) text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GBSearch {
    #[serde(rename(serialize = "_idRow", deserialize = "_idRow"))]
    id: u64,
    #[serde(rename(serialize = "_sModelName", deserialize = "_sModelName"))]
    model_name: String,
    #[serde(rename(serialize = "_sSingularTitle", deserialize = "_sSingularTitle"))]
    title: String,
    #[serde(rename(serialize = "_sIconClasses", deserialize = "_sIconClasses"))]
    icon_classes: String,
    #[serde(rename(serialize = "_sName", deserialize = "_sName"))]
    name: String,
    #[serde(rename(serialize = "_sProfileUrl", deserialize = "_sProfileUrl"))]
    profile_url: String,
    #[serde(rename(serialize = "_tsDateAdded", deserialize = "_tsDateAdded"))]
    date_added: u64,
    #[serde(rename(serialize = "_bHasFiles", deserialize = "_bHasFiles"))]
    has_files: bool,
    #[serde(rename(serialize = "_aSubmitter", deserialize = "_aSubmitter"))]
    submitter: GbSubmitter,
    #[serde(rename(serialize = "_tsDateUpdated", deserialize = "_tsDateUpdated"), default)]
    date_updated: u64,
    #[serde(rename(serialize = "_bIsNsfw", deserialize = "_bIsNsfw"))]
    is_nsfw: bool,
    #[serde(rename(serialize = "_sInitialVisibility", deserialize = "_sInitialVisibility"))]
    initial_visibility: String,
    #[serde(rename(serialize = "_nLikeCount", deserialize = "_nLikeCount"), default)]
    like_count: i32,
    #[serde(rename(serialize = "_nPostCount", deserialize = "_nPostCount"), default)]
    post_count: i32,
    #[serde(rename(serialize = "_bWasFeatured", deserialize = "_bWasFeatured"))]
    was_featured: bool,
    #[serde(rename(serialize = "_nViewCount", deserialize = "_nViewCount"))]
    view_count: i32,
    #[serde(rename(serialize = "_bIsOwnedByAccessor", deserialize = "_bIsOwnedByAccessor"))]
    is_owned_by_accessor: bool,
    #[serde(rename(serialize = "_aPreviewMedia", deserialize = "_aPreviewMedia"))]
    preview_media: GbPreview,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GbSubmitter {
    #[serde(rename(serialize = "_idRow", deserialize = "_idRow"))]
    id: u64,
    #[serde(rename(serialize = "_sName", deserialize = "_sName"))]
    name: String,
    #[serde(rename(serialize = "_bIsOnline", deserialize = "_bIsOnline"))]
    is_online: bool,
    #[serde(rename(serialize = "_bHasRipe", deserialize = "_bHasRipe"))]
    has_ripe: bool,
    #[serde(rename(serialize = "_sProfileUrl", deserialize = "_sProfileUrl"))]
    profile_url: String,
    #[serde(rename(serialize = "_sAvatarUrl", deserialize = "_sAvatarUrl"))]
    avatar_rl: String,
    #[serde(rename(serialize = "_sUpicUrl", deserialize = "_sUpicUrl"), default)]
    upic_url: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GbPreview {
    #[serde(rename(serialize = "_aImages", deserialize = "_aImages"))]
    images: Vec<GbPreviewImage>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GbPreviewImage {
    #[serde(rename(serialize = "_sType", deserialize = "_sType"))]
    img_type: String,
    #[serde(rename(serialize = "_sBaseUrl", deserialize = "_sBaseUrl"))]
    base_url: String,
    #[serde(rename(serialize = "_sFile", deserialize = "_sFile"))]
    file: String,
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
            let res: Result<sonic_rs::Value, Error> = sonic_rs::from_str(data_json.as_str());
            let mut dl_files: Vec<GbModDownload> = Vec::new();
            if res.is_ok() {
                let mod_data = res.unwrap();
                let info_mods = mod_data[1].clone().into_object();
                // make sure we've actually got a proper response data
                if info_mods.is_some() {
                    for (_key, value) in info_mods.unwrap().iter() {
                        let dl_file: GbModDownload = sonic_rs::from_value(value).unwrap();
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

pub fn _download_mod_file(gb_file: &GbModDownload, sender: Sender<DlFinish>) -> Receiver<u64> {
    println!("{}", gb_file.file);
    let (tx, rx) = channel::<u64>(2048);
    let gb_file = gb_file.clone();
    tokio::spawn(async move {
        let mut dst = Vec::new();
        let mut easy = Easy::new();
        easy.url(gb_file.download_url.as_str()).unwrap();
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
        let mut file_res = File::create("/tmp/rust4diva/".to_owned() + &gb_file.file);
        match file_res {
            Ok(mut file) => {
                println!("Writing file");
                file.write_all(dst.as_slice()).expect("Failed to write to file");
                sender.send(DlFinish {
                    file: gb_file,
                    success: true,
                }).await.unwrap();
            }
            Err(_e) => {
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

        let res = reqwest::get(&gb_dl.download_url).await;
        match res {
            Ok(res) => {
                println!("Has received response from GameBanana");
                let mut stream = &mut res.bytes_stream();
                let mut handle = File::create("/tmp/rust4diva/".to_owned() + &*gb_dl.file);
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
                        if downloaded_size < gb_file.filesize as u64 {
                            all_good = false;
                        }
                        if all_good {
                            println!("Saving file to disk: {}", &gb_dl.file);
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
pub async fn init(ui: &App, diva_arc: Arc<Mutex<DivaData>>, dl_tx: Sender<(i32, Download)>) {
    let file_arc: Arc<Mutex<Vec<GbModDownload>>> = Arc::new(Mutex::new(Vec::new()));

    // setup thread for downloading, this will listen for Download objects


    let search_diva = Arc::clone(&diva_arc);
    let ui_search_handle = ui.as_weak();
    ui.on_search_gb(move |search| {
        println!("Searching for {}", search);
        search_mods(search.parse().unwrap(), &search_diva, ui_search_handle.clone());
    });

    let file_diva = Arc::clone(&diva_arc);
    let ui_file_handle = ui.as_weak();
    let list_files = Arc::clone(&file_arc);
    ui.on_list_files(move |mod_row| {
        get_mod_files(mod_row, &file_diva, &list_files, ui_file_handle.clone());
    });

    let download_diva = Arc::clone(&diva_arc);
    let download_files = Arc::clone(&file_arc);
    let ui_download_handle = ui.as_weak();
    ui.on_download_file(move |file, file_row| {
        println!("Download: {}, {}", file.name, file_row);
        let ui_file = file.clone();
        let _ = ui_download_handle.clone().upgrade_in_event_loop(move |ui| {
            let file = ui_file.clone();
            let downloads = ui.get_downloads_list();
            let dc = downloads.as_any().downcast_ref::<VecModel<Download>>();
            match dc {
                Some(mut downloads) => {
                    println!("Pushing");
                    downloads.push(file);
                }
                None => {
                    println!("wasn't able to downcast wtf");
                }
            }
        });
        // download_mod_file(file_row, &download_diva, ui_download_handle.clone(), file);
        let _ = dl_tx.clone().try_send((file_row, file)).unwrap();
    });

    // ui.on_download_file()
}
pub fn search_mods(search: String, search_diva: &Arc<Mutex<DivaData>>, ui_search_handle: Weak<App>) {
    let search_diva = Arc::clone(search_diva);
    ui_search_handle.upgrade_in_event_loop(move |ui| {
        ui.set_s_prog_vis(true);
    }).expect("TODO: panic message");

    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let req = client.get(format!("{}/{}", GB_DOMAIN, GB_MOD_SEARCH))
            .query(&[("_idGameRow", GB_DIVA_ID.to_string()), ("_sName", search), ("_nPerpage", "50".to_string())]);
        match req.send().await {
            Ok(res) => {
                match res.text().await {
                    Ok(res_as_text) => {
                        println!("Text received");
                        let results: Result<Value, Error> = sonic_rs::from_str(res_as_text.as_str());
                        match results {
                            Ok(val) => {
                                // println!("THIS IS THE VALUE PRINT\n{:?}", val);
                                if let Some(vals) = val.as_array() {
                                    println!("Array time");
                                    let mut search_results = Vec::new();
                                    for item in vals.iter() {
                                        match sonic_rs::from_value::<GBSearch>(item) {
                                            // if let Ok(search_item) =  {
                                            Ok(search_item) => {
                                                search_results.push(search_item);
                                            }
                                            Err(e) => {
                                                println!("{:?}", sonic_rs::to_string(item));
                                                eprintln!("{}", e);
                                            }
                                        }
                                    }
                                    let mut diva = search_diva.lock().await;
                                    diva.search_results = search_results.clone();
                                    ui_search_handle.upgrade_in_event_loop(move |ui| {
                                        println!("setting stuff");
                                        let model_vec: VecModel<ModelRc<StandardListViewItem>> = VecModel::default();
                                        for item in search_results {
                                            println!("{}", item.name);
                                            let items: Rc<VecModel<StandardListViewItem>> = Rc::new(VecModel::default());
                                            let name = StandardListViewItem::from(item.name.as_str());
                                            let category = StandardListViewItem::from(item.model_name.as_str());
                                            let author = StandardListViewItem::from(item.submitter.name.as_str());
                                            items.push(name);
                                            items.push(author);
                                            items.push(category);
                                            model_vec.push(items.into());
                                        }
                                        let model = ModelRc::new(model_vec);
                                        ui.set_search_results(model);
                                    }).expect("crashed bitch");
                                }
                            }
                            Err(_) => {}
                        }
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("{}", e);
            }
        }
        ui_search_handle.upgrade_in_event_loop(move |ui| {
            println!("Search done");
            ui.set_s_prog_vis(false);
        }).expect("Something got borked @ gba 377");
    });
}


pub fn get_mod_files(mod_row: i32, file_diva: &Arc<Mutex<DivaData>>, files: &Arc<Mutex<Vec<GbModDownload>>>, ui_file_handle: Weak<App>) {
    let file_diva = Arc::clone(file_diva);
    let files = files.clone();
    ui_file_handle.upgrade_in_event_loop(move |ui| {
        ui.set_s_prog_vis(true);
    }).expect("TODO: panic message");
    tokio::spawn(async move {
        let mut diva = file_diva.lock().await;
        if mod_row < (diva.search_results.len() as i32) && diva.search_results.len() != 0 {
            let module = diva.search_results[mod_row as usize].clone();
            if diva.mod_files.contains_key(&module.id) {
                if let Some(gb) = diva.mod_files.get(&module.id) {
                    set_files_list(ui_file_handle.clone(), gb);
                    let mut files = files.lock().await;
                    // files.c
                    *files = gb.clone();
                }
                ui_file_handle.upgrade_in_event_loop(move |ui| {
                    ui.set_s_prog_vis(false);
                }).expect("TODO: panic message");
                // return early, we don't need to fetch from gb since we already have them loaded
                return;
            }


            let gb_module = fetch_mod_data(module.id.to_string().as_str());
            if let Some(gb) = gb_module {
                diva.mod_files.insert(module.id, gb.files.clone());
                set_files_list(ui_file_handle.clone(), &gb.files);
                let mut files = files.lock().await;
                *files = gb.files.clone();
            }
        }
        ui_file_handle.upgrade_in_event_loop(move |ui| {
            ui.set_s_prog_vis(false);
        }).expect("TODO: panic message");
    });
}

pub fn set_files_list(ui_handle: Weak<App>, files: &Vec<GbModDownload>) {
    let files = files.clone();
    ui_handle.upgrade_in_event_loop(move |ui| {
        let model_vec = VecModel::default();
        for item in files {
            model_vec.push(Download {
                id: item.id as i32,
                url: SharedString::from(item.download_url),
                name: SharedString::from(item.file),
                size: item.filesize as i32,
                progress: 0.0,
            });
        }
        let model = ModelRc::new(model_vec);
        ui.set_file_results(model);
    }).expect("TODO: panic message");
}

pub fn do_download_queue(q: MutexGuard<VecDeque<(i32, Download)>>, ui_download_handle: Weak<App>) {}


pub fn download_mod_file(file_idx: i32, download_diva: &Arc<Mutex<DivaData>>, ui_download_handle: Weak<App>, download: Download) {
    // let files = Arc::clone(files);
    let download = download.clone();
    tokio::spawn(async move {
        let download = download.clone();
        // let lfiles = files.lock().await;
        // make channels for sending the progress between
        let (prog_tx, mut prog_rx) = channel::<i32>(204800);

        // actually do the download
        tokio::spawn(async move {
            let prog_tx = prog_tx.clone();
            println!("setting up the download");

            let mut easy = Easy::new();
            let mut dst = Vec::new();
            easy.url(download.url.as_str()).unwrap();
            let mut started = false;

            let _redirect = easy.follow_location(true);
            {
                let mut transfer = easy.transfer();
                transfer.write_function(|data| {
                    if !started {
                        started = true;
                        println!("First chunk received");
                    }
                    dst.extend_from_slice(data);
                    let p = prog_tx.try_send(data.len() as i32);
                    match p {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("{}", e);
                        }
                    }
                    Ok(data.len())
                }).unwrap();
                transfer.perform().unwrap();
            }
            let mut file_res = File::create("/tmp/rust4diva/".to_owned() + &download.name);
            // files
            match file_res {
                Ok(mut f) => {
                    match f.write_all(dst.as_slice()) {
                        Ok(_) => {
                            println!("Saved successfully, will try to extract");
                        }
                        Err(e) => {
                            eprintln!("Something went wrong while saving the file to disk \n{}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Something went wrong while saving the file to disk \n{}", e);
                }
            }
        });

        // update ui with progress
        tokio::spawn(async move {
            let mut downloaded = 0;
            // let file = files.clone();
            let file_size = download.size;
            while !prog_rx.is_closed() {
                // println!("Waiting");
                match prog_rx.try_recv() {
                    Err(e) => {
                        // println!("nothing rx'd");
                    }
                    Ok(chunk_size) => {
                        downloaded += chunk_size;
                        let progress = (downloaded as f32) / (file_size as f32);
                        ui_download_handle.upgrade_in_event_loop(move |ui| {
                            let downloads = ui.get_downloads_list();
                            if let Some(downloads) = downloads.as_any().downcast_ref::<VecModel<Download>>() {
                                if let Some(mut download) = downloads.row_data(file_idx as usize) {
                                    download.progress = progress;
                                    downloads.set_row_data(file_idx as usize, download);
                                }
                            }
                        }).expect("");
                    }
                }
            }
            println!("Download channel closed");
        });
    });
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
