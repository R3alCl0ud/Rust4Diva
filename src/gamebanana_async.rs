use std::collections::HashMap;
use std::error::Error;
use std::io::BufRead;
use std::sync::Arc;

use curl::easy::Easy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use slint::{
    ComponentHandle, Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel,
    Weak,
};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;

use crate::diva::open_error_window;
use crate::{App, DivaData, Download, GameBananaLogic, GbPreviewData};

const GB_API_DOMAIN: &str = "https://api.gamebanana.com";
const GB_DOMAIN: &str = "https://gamebanana.com";
const GB_DIVA_ID: i32 = 16522;

const GB_MOD_INFO: &str = "/Core/Item/Data";
const GB_MOD_SEARCH: &str = "apiv11/Game/16522/Subfeed";

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
    #[serde(rename(
        serialize = "_sAnalysisResultCode",
        deserialize = "_sAnalysisResultCode"
    ))]
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
    #[serde(
        rename(serialize = "_tsDateUpdated", deserialize = "_tsDateUpdated"),
        default
    )]
    date_updated: u64,
    #[serde(rename(serialize = "_bIsNsfw", deserialize = "_bIsNsfw"), default)]
    is_nsfw: bool,
    #[serde(rename(serialize = "_sInitialVisibility", deserialize = "_sInitialVisibility"))]
    initial_visibility: String,
    #[serde(
        rename(serialize = "_nLikeCount", deserialize = "_nLikeCount"),
        default
    )]
    like_count: i32,
    #[serde(
        rename(serialize = "_nPostCount", deserialize = "_nPostCount"),
        default
    )]
    post_count: i32,
    #[serde(
        rename(serialize = "_bWasFeatured", deserialize = "_bWasFeatured"),
        default
    )]
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
    #[serde(rename(serialize = "_aImages", deserialize = "_aImages"), default)]
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

#[derive(Clone, Debug)]
pub struct GbDmmItem {
    pub item_id: String,
    pub itemtype: String,
    pub file_id: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GbSearchResults {
    #[serde(rename(serialize = "_aMetadata", deserialize = "_aMetadata"))]
    metadata: GbMetadata,
    #[serde(rename(serialize = "_aRecords", deserialize = "_aRecords"))]
    records: Vec<GBSearch>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GbMetadata {
    #[serde(rename(serialize = "_nRecordCount", deserialize = "_nRecordCount"))]
    record_count: i32,
    #[serde(rename(serialize = "_nPerpage", deserialize = "_nPerpage"))]
    perpage: i32,
    #[serde(rename(serialize = "_bIsComplete", deserialize = "_bIsComplete"))]
    is_complete: bool,
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
    easy.url(
        format!(
            "{}{}?itemid={}&itemtype=Mod&fields=name,Files().aFiles()",
            GB_API_DOMAIN, GB_MOD_INFO, &mod_id
        )
        .as_str(),
    )
    .unwrap();
    {
        let mut transfer = easy.transfer();
        transfer
            .write_function(|data| {
                let mut data_json: String = String::new();
                for line in data.lines() {
                    data_json.push_str(line.unwrap().as_str());
                }
                // println!("{:#}", data_json);
                let res = sonic_rs::from_str::<sonic_rs::Value>(data_json.as_str());
                let mut dl_files: Vec<GbModDownload> = Vec::new();
                if let Ok(mod_data) = res {
                    let info_mods = mod_data[1].clone().into_object();
                    // make sure we've actually got a proper response data
                    if let Some(info_mods) = info_mods {
                        for (_key, value) in info_mods.iter() {
                            let dl_file = sonic_rs::from_value::<GbModDownload>(value).unwrap();
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
            })
            .expect("TODO: panic message");
        transfer.perform().unwrap();
    }
    easy.perform().unwrap();
    return the_mod;
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
        file_id: m_info.get(1).unwrap().as_str().to_string(),
        itemtype: m_info.get(2).unwrap().as_str().to_string(),
        item_id: m_info.get(3).unwrap().as_str().to_string(),
    });
}

pub async fn init(ui: &App, dl_tx: Sender<(i32, Download)>, url_rx: Receiver<String>) {
    let ui_search_handle = ui.as_weak();
    let file_arc: Arc<Mutex<Vec<GbModDownload>>> = Arc::new(Mutex::new(Vec::new()));

    let search_res: Arc<std::sync::Mutex<HashMap<i32, GBSearch>>> =
        Arc::new(std::sync::Mutex::new(HashMap::new()));

    ui.global::<GameBananaLogic>()
        .on_search(move |search, page| {
            let ui_search_handle = ui_search_handle.clone();
            let ui_result_handle = ui_search_handle.clone();
            ui_search_handle.unwrap().set_s_prog_vis(true);
            tokio::spawn(async move {
                match search_gb(search.to_string(), page.clone()).await {
                    Ok(res) => {
                        let _ = ui_result_handle.upgrade_in_event_loop(move |ui| {
                            let mut items = vec![];
                            for i in res.records.clone() {
                                items.push(GbPreviewData {
                                    author: i.submitter.name.into(),
                                    id: i.id as i32,
                                    image: slint::Image::default(),
                                    item_type: i.model_name.into(),
                                    name: i.name.into(),
                                    updated: "Never".into(),
                                });
                            }
                            if page == 1 {
                                ui.set_s_results(ModelRc::new(VecModel::from(items.clone())));
                                ui.set_n_results(res.metadata.record_count);
                            } else {
                                let model = ui.get_s_results();
                                let results = match model
                                    .as_any()
                                    .downcast_ref::<VecModel<GbPreviewData>>()
                                {
                                    Some(vec) => vec,
                                    None => {
                                        ui.set_s_prog_vis(false);
                                        return;
                                    }
                                };
                                for i in items {
                                    results.push(i);
                                }
                            }
                            ui.set_s_prog_vis(false);
                            for i in res.records.clone() {
                                let weak = ui.as_weak();
                                tokio::spawn(async move {
                                    get_preview_image(weak.clone(), i.clone()).await;
                                });
                            }
                        });
                    }
                    Err(e) => open_error_window(e.to_string()),
                }
            });
        });

    // let ui_file_handle = ui.as_weak();
    ui.on_list_files(move |_mod_row| {
        // get_mod_files(mod_row, &file_diva, &list_files, ui_file_handle.clone());
    });

    let ui_download_handle = ui.as_weak();
    let oneclick_tx = dl_tx.clone();
    ui.on_download_file(move |file, file_row| {
        let ui_file = file.clone();
        let _ = ui_download_handle.clone().upgrade_in_event_loop(move |ui| {
            let file = ui_file.clone();
            let downloads = ui.get_downloads_list();
            let dc = downloads.as_any().downcast_ref::<VecModel<Download>>();
            match dc {
                Some(downloads) => {
                    downloads.push(file);
                }
                None => {
                    println!("wasn't able to downcast wtf");
                }
            }
        });
        let _ = dl_tx.clone().try_send((file_row, file));
    });
    let ui_oneclick_handle = ui.as_weak();
    let _ = handle_dmm_oneclick(url_rx, ui_oneclick_handle, oneclick_tx.clone());
}

pub fn get_mod_files(
    mod_row: i32,
    file_diva: &Arc<Mutex<DivaData>>,
    files: &Arc<Mutex<Vec<GbModDownload>>>,
    ui_file_handle: Weak<App>,
) {
    let file_diva = Arc::clone(file_diva);
    let files = files.clone();
    ui_file_handle
        .upgrade_in_event_loop(move |ui| {
            ui.set_s_prog_vis(true);
        })
        .expect("gba 323");
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
                ui_file_handle
                    .upgrade_in_event_loop(move |ui| {
                        ui.set_s_prog_vis(false);
                    })
                    .expect("Failed to update progress");
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
        ui_file_handle
            .upgrade_in_event_loop(move |ui| {
                ui.set_s_prog_vis(false);
            })
            .expect("TODO: panic message");
    });
}

pub fn set_files_list(ui_handle: Weak<App>, files: &Vec<GbModDownload>) {
    let files = files.clone();
    ui_handle
        .upgrade_in_event_loop(move |ui| {
            let model_vec = VecModel::default();
            for item in files {
                model_vec.push(Download {
                    id: item.id as i32,
                    url: SharedString::from(item.download_url),
                    name: SharedString::from(item.file),
                    size: item.filesize as i32,
                    progress: 0.0,
                    failed: false,
                });
            }
            let model = ModelRc::new(model_vec);
            ui.set_file_results(model);
        })
        .expect("TODO: panic message");
}

pub fn handle_dmm_oneclick(
    mut url_rx: Receiver<String>,
    ui_handle: Weak<App>,
    sender: Sender<(i32, Download)>,
) -> tokio::task::JoinHandle<()> {
    return tokio::spawn(async move {
        while !url_rx.is_closed() {
            match url_rx.recv().await {
                Some(url) => {
                    println!("GB @ 458: {}", url);
                    if let Some(oneclick) = parse_dmm_url(url) {
                        println!("Parsed successfully, fetching info now");

                        if let Some(mod_info) = fetch_mod_data(oneclick.item_id.as_str()) {
                            // let mut file_iter = ;
                            println!("Mod: {}", mod_info.name);
                            let files = mod_info.files.clone();
                            let files = files
                                .iter()
                                .cloned()
                                .find(|file| file.id.to_string() == oneclick.file_id);

                            if let Some(file) = files {
                                println!("Found the right file: {}", file.file);
                                let ui_sender = sender.clone();
                                let _ = ui_handle.upgrade_in_event_loop(move |ui| {
                                    let downloads = ui.get_downloads_list();
                                    let dc =
                                        downloads.as_any().downcast_ref::<VecModel<Download>>();
                                    match dc {
                                        Some(downloads) => {
                                            println!("Pushing");
                                            let cur_len = downloads.iter().len();
                                            let download = Download {
                                                id: file.id as i32,
                                                name: SharedString::from(&file.file),
                                                progress: 0.0,
                                                size: file.filesize as i32,
                                                url: SharedString::from(&file.download_url),
                                                failed: false,
                                            };
                                            downloads.push(download.clone());
                                            match ui_sender.try_send((cur_len as i32, download)) {
                                                Ok(_) => {}
                                                Err(e) => {
                                                    println!("{}", e);
                                                }
                                            }
                                        }
                                        None => {
                                            println!("wasn't able to downcast wtf");
                                        }
                                    }
                                });
                            }
                        } else {
                            println!("Unable to get the info for some reason");
                        }
                    }
                }
                None => {}
            }
        }
        println!("Oneclick receiver closed");
    });
}

pub async fn get_preview_image(weak: Weak<App>, item: GBSearch) {
    if let Some(preview) = item.preview_media.images.first() {
        let client = reqwest::Client::new();
        let req = client.get(format!("{}/{}", preview.base_url, preview.file));
        if let Ok(res) = req.send().await {
            if let Ok(bytes) = res.bytes().await {
                let image = match image::load_from_memory(&bytes) {
                    Ok(image) => image,
                    Err(e) => {
                        eprintln!("{e}");
                        // open_error_window(e.to_string());
                        return;
                    }
                };
                let image = image
                    .resize(220 as u32, 124 as u32, image::imageops::FilterType::Nearest)
                    .into_rgba8();
                let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                    image.as_raw(),
                    image.width(),
                    image.height(),
                );
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    let image = slint::Image::from_rgba8(buffer);
                    let model = ui.get_s_results();
                    let results = match model.as_any().downcast_ref::<VecModel<GbPreviewData>>() {
                        Some(results) => results,
                        None => {
                            return;
                        }
                    };
                    for i in 0..results.row_count() {
                        let mut row = results.row_data(i).unwrap();
                        if row.id == item.id as i32 {
                            row.image = image;
                            results.set_row_data(i, row);
                            return;
                        }
                    }
                });
            }
        }
        //.get(format!("{GB_API_DOMAIN}/{GB_MOD_INFO}")).query(&[("itemid", item.id), ("fields", "Files().aFiles()")]);
    }
}

pub async fn search_gb(
    search: String,
    page: i32,
) -> Result<GbSearchResults, Box<dyn Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let req = client
        .get(format!("{GB_DOMAIN}/{GB_MOD_SEARCH}"))
        .query(&[("_sName", search), ("_nPage", page.to_string())]);
    // req.
    let res = req.send().await?.text().await?;
    Ok(sonic_rs::from_str::<GbSearchResults>(&res)?)
}
