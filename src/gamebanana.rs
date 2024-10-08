use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use chrono::DateTime;
use futures_util::StreamExt;
use regex::Regex;
use serde::{Deserialize, Serialize};

use slint::private_unstable_api::re_exports::ColorScheme;
use tokio::sync::broadcast;
use tokio::time::sleep;
// use slint::Pal
use crate::diva::{get_temp_folder, open_error_window};
use crate::modmanagement::{get_mods, load_mods, set_mods_table, unpack_mod_path};
use crate::util::reqwest_client;
use crate::{
    App, Download, GameBananaLogic, GbDetailsWindow, GbPreviewData, HyperLink, SlGbSubmitter,
    R4D_CFG,
};
use slint::{ComponentHandle, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel, Weak};
use tokio::sync::mpsc::{channel, Receiver};

const GB_DOMAIN: &str = "https://gamebanana.com";
const GB_DIVA_ID: i32 = 16522;
const GB_MOD_DATA: &'static str = "apiv11/Mod";
const GB_MOD_SEARCH: &str = "apiv11/Util/Search/Results";
#[allow(dead_code)]
const GB_DIVA_SUBFEED: &str = "apiv11/Game/16522/Subfeed";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GbModDownload {
    #[serde(rename(serialize = "_idRow", deserialize = "_idRow"))]
    pub id: i32,
    #[serde(rename(serialize = "_sFile", deserialize = "_sFile"))]
    pub file: String,
    #[serde(rename(serialize = "_nFilesize", deserialize = "_nFilesize"))]
    pub filesize: u32,
    #[serde(rename(serialize = "_sDescription", deserialize = "_sDescription"))]
    pub description: String,
    #[serde(rename(serialize = "_tsDateAdded", deserialize = "_tsDateAdded"))]
    pub date_added: u32,
    #[serde(rename(serialize = "_nDownloadCount", deserialize = "_nDownloadCount"))]
    pub download_count: u32,
    #[serde(rename(serialize = "_sMd5Checksum", deserialize = "_sMd5Checksum"))]
    pub md5_checksum: String,
    #[serde(rename(serialize = "_sDownloadUrl", deserialize = "_sDownloadUrl"))]
    pub download_url: String,
    #[serde(rename(serialize = "_sClamAvResult", deserialize = "_sClamAvResult"))]
    pub clam_av_result: String,
    #[serde(rename(deserialize = "_sAvastAvResult"))]
    pub avast_av_result: String,
    #[serde(rename(deserialize = "_sAnalysisState"))]
    pub analysis_state: String,
    #[serde(rename(deserialize = "_sAnalysisResult"))]
    pub analysis_result: String,
    #[serde(rename(deserialize = "_sAnalysisResultCode"))]
    pub analysis_result_code: String,
    #[serde(rename(serialize = "_bContainsExe", deserialize = "_bContainsExe"))]
    pub contains_exe: bool,
}

impl From<GbModDownload> for Download {
    fn from(value: GbModDownload) -> Self {
        Self {
            failed: false,
            id: value.id as i32,
            name: value.file.into(),
            progress: 0,
            size: value.filesize as i32,
            url: value.download_url.into(),
            inprogress: false,
        }
    }
}

impl PartialEq<i32> for Download {
    fn eq(&self, other: &i32) -> bool {
        self.id == *other
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GbMod {
    #[serde(rename(deserialize = "_sName"))]
    pub name: String,
    #[serde(rename(deserialize = "_aFiles"))]
    pub files: Option<Vec<GbModDownload>>,
    #[serde(rename(deserialize = "_sText"))]
    pub text: Option<String>,
    #[serde(rename(deserialize = "_aSubmitter"))]
    pub submitter: Option<GbSubmitter>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GBSearch {
    #[serde(rename(deserialize = "_idRow"))]
    id: u64,
    #[serde(rename(deserialize = "_sModelName"), default)]
    model_name: String,
    #[serde(rename(deserialize = "_sSingularTitle"), default)]
    title: String,
    #[serde(rename(deserialize = "_sIconClasses"), default)]
    icon_classes: String,
    #[serde(rename(deserialize = "_sName"), default)]
    name: String,
    #[serde(rename(deserialize = "_sProfileUrl"), default)]
    profile_url: String,
    #[serde(rename(deserialize = "_tsDateAdded"))]
    date_added: i64,
    #[serde(rename(deserialize = "_bHasFiles"), default)]
    has_files: bool,
    #[serde(rename(deserialize = "_aSubmitter"))]
    submitter: GbSubmitter,
    #[serde(rename(deserialize = "_tsDateUpdated"), default)]
    date_updated: i64,
    #[serde(rename(deserialize = "_bIsNsfw"), default)]
    is_nsfw: bool,
    #[serde(rename(deserialize = "_sInitialVisibility"), default)]
    initial_visibility: String,
    #[serde(rename(deserialize = "_nLikeCount"), default)]
    like_count: i32,
    #[serde(rename(deserialize = "_nPostCount"), default)]
    post_count: i32,
    #[serde(rename(deserialize = "_bWasFeatured"), default)]
    was_featured: bool,
    #[serde(rename(deserialize = "_nViewCount"), default)]
    view_count: i32,
    #[serde(rename(deserialize = "_bIsOwnedByAccessor"), default)]
    is_owned_by_accessor: bool,
    #[serde(rename(deserialize = "_aPreviewMedia"))]
    preview_media: GbPreview,
    #[serde(rename(deserialize = "_aFiles"), default)]
    files: Vec<GbModDownload>,
}

impl From<GBSearch> for GbPreviewData {
    fn from(value: GBSearch) -> Self {
        let mut imgurl = "".to_owned();
        if let Some(img) = value.preview_media.images.first() {
            imgurl = format!("{}/{}", img.base_url, img.file);
        }
        let mut updated = "Never".to_owned();
        if value.date_updated != 0 {
            let date =
                DateTime::<chrono::Utc>::from_timestamp(value.date_updated as i64, 0).unwrap();
            updated = date.format("%d/%m/%Y %H:%M").to_string();
        }
        let mut added = "Never".to_owned();
        if value.date_added != 0 {
            let date = DateTime::<chrono::Utc>::from_timestamp(value.date_added as i64, 0).unwrap();
            added = date.format("%d/%m/%Y %H:%M").to_string();
        }
        Self {
            author: value.submitter.into(),
            id: value.id as i32,
            image: Default::default(),
            item_type: value.model_name.into(),
            name: value.name.into(),
            updated: updated.into(),
            image_url: imgurl.into(),
            image_loaded: false,
            submitted: added.into(),
        }
    }
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
    avatar_url: String,
    #[serde(rename(serialize = "_sUpicUrl", deserialize = "_sUpicUrl"), default)]
    upic_url: String,
}

impl From<GbSubmitter> for SlGbSubmitter {
    fn from(submitter: GbSubmitter) -> SlGbSubmitter {
        SlGbSubmitter {
            name: submitter.name.into(),
            avatar_url: submitter.avatar_url.into(),
        }
    }
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
    pub item_id: i32,
    #[allow(dead_code)]
    pub itemtype: String,
    pub file_id: i32,
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

pub enum GbSearchSort {
    Relevance(String),
    Popularity(String),
    Newest(String),
    Updated(String),
}

impl Default for GbSearchSort {
    fn default() -> Self {
        Self::Relevance("best_match".to_owned())
    }
}

impl Into<String> for GbSearchSort {
    fn into(self) -> String {
        match self {
            GbSearchSort::Relevance(s) => s,
            GbSearchSort::Popularity(s) => s,
            GbSearchSort::Newest(s) => s,
            GbSearchSort::Updated(s) => s,
        }
    }
}

impl From<i32> for GbSearchSort {
    fn from(value: i32) -> Self {
        match value {
            1 => Self::Popularity("popularity".to_owned()),
            2 => Self::Newest("date".to_owned()),
            3 => Self::Updated("udate".to_owned()),
            _ => Self::default(),
        }
    }
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
        file_id: m_info.get(1).unwrap().as_str().parse().unwrap(),
        itemtype: m_info.get(2).unwrap().as_str().to_string(),
        item_id: m_info.get(3).unwrap().as_str().parse().unwrap(),
    });
}

pub async fn init(ui: &App, url_rx: Receiver<String>, dark_rx: broadcast::Receiver<ColorScheme>) {
    let ui_search_handle = ui.as_weak();

    ui.global::<GameBananaLogic>()
        .on_search(move |search, page, sort| {
            let ui_search_handle = ui_search_handle.clone();
            let ui_result_handle = ui_search_handle.clone();
            ui_search_handle.unwrap().set_s_prog_vis(true);
            tokio::spawn(async move {
                match search_gb(search.to_string(), page.clone(), sort.clone()).await {
                    Ok(res) => {
                        let _ = ui_result_handle.upgrade_in_event_loop(move |ui| {
                            let mut items = vec![];
                            for i in res.records.clone() {
                                items.push(i.into());
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
                                    get_and_set_preview_image(weak.clone(), i.clone()).await;
                                });
                            }
                        });
                    }
                    Err(e) => open_error_window(e.to_string()),
                }
            });
        });

    let weak = ui.as_weak();
    let darkrrx = dark_rx.resubscribe();
    ui.global::<GameBananaLogic>().on_list_files(move |item| {
        let weak = weak.clone();
        let dark_rx = darkrrx.resubscribe();
        let deets = create_deets_window(item, weak, dark_rx);
        deets.show().unwrap();
    });
    let ui_oneclick_handle = ui.as_weak();
    let _ = handle_dmm_oneclick(url_rx, ui_oneclick_handle, dark_rx.resubscribe());
}

pub fn handle_dmm_oneclick(
    mut url_rx: Receiver<String>,
    ui_handle: Weak<App>,
    dark_rx: broadcast::Receiver<ColorScheme>,
) -> tokio::task::JoinHandle<()> {
    return tokio::spawn(async move {
        while !url_rx.is_closed() {
            if let Some(url) = url_rx.recv().await {
                let item = match parse_dmm_url(url) {
                    Some(item) => item,
                    None => continue,
                };
                let m = match fetch_mod(item.item_id).await {
                    Ok(m) => m,
                    Err(e) => {
                        open_error_window(e.to_string());
                        continue;
                    }
                };
                let weak = ui_handle.clone();
                let rx = dark_rx.resubscribe();
                let _ = slint::invoke_from_event_loop(move || {
                    let deets = create_deets_window(m.clone().into(), weak, rx);
                    let files: VecModel<Download> = VecModel::default();
                    for file in m.files.clone() {
                        let mut f: Download = file.clone().into();
                        if f.id == item.file_id {
                            f.inprogress = true;
                        }
                        files.push(f);
                    }
                    deets.set_files(ModelRc::new(files));
                    if let Some(file) = m.files.iter().find(|f| f.id == item.file_id) {
                        deets
                            .global::<GameBananaLogic>()
                            .invoke_download(file.clone().into());
                    }
                    deets.show().unwrap();
                });
                // cre
            }
        }
        println!("Oneclick receiver closed");
    });
}

pub async fn get_and_set_preview_image(weak: Weak<App>, item: GBSearch) {
    let mut buffer = missing_image_buf();
    if let Some(preview) = item.preview_media.images.first() {
        if let Ok(buf) = get_image(format!("{}/{}", preview.base_url, preview.file)).await {
            buffer = buf;
        }
    }
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
                row.image_loaded = true;
                results.set_row_data(i, row);
                return;
            }
        }
    });
}

pub async fn get_image(
    url: String,
) -> Result<SharedPixelBuffer<Rgba8Pixel>, Box<dyn Error + Sync + Send>> {
    let client = reqwest::Client::new();
    let req = client.get(url);
    let res = req.send().await?;
    let bytes = res.bytes().await?;
    let image = image::load_from_memory(&bytes)?;
    let image = image
        .resize(880 as u32, 496 as u32, image::imageops::FilterType::Nearest)
        .into_rgba8();
    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        image.as_raw(),
        image.width(),
        image.height(),
    );
    Ok(buffer)
}

pub async fn search_gb(
    search: String,
    page: i32,
    sort: i32,
) -> Result<GbSearchResults, Box<dyn Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let req = client.get(format!("{GB_DOMAIN}/{GB_MOD_SEARCH}")).query(&[
        ("_sSearchString", search),
        ("_nPage", page.to_string()),
        ("_nPerpage", "30".to_owned()),
        ("_sOrder", GbSearchSort::from(sort).into()),
        ("_idGameRow", GB_DIVA_ID.to_string()),
        ("_sModelName", "Mod".to_owned()),
    ]);
    // req.
    let res = req.send().await?.text().await?;
    match sonic_rs::from_str::<GbSearchResults>(&res) {
        Ok(results) => Ok(results),
        Err(e) => {
            eprintln!("{}", res); // log the res that failed to parse
            Err(e.into())
        }
    }
    // Ok(sonic_rs::from_str::<GbSearchResults>(&res)?)
}

pub async fn fetch_mod_info(mod_id: i32) -> Result<GbMod, Box<dyn Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let req = client.get(format!(
        "{}/{}/{}?_csvProperties=_aFiles,_sText,_idRow,_sName,_aSubmitter",
        GB_DOMAIN,
        GB_MOD_DATA,
        mod_id.clone()
    ));
    let text = req.send().await?.text().await?;
    match sonic_rs::from_str::<GbMod>(&text) {
        Ok(module) => Ok(module),
        Err(e) => {
            eprintln!("{}", text); // log the res that failed to parse
            Err(e.into())
        }
    }
}

pub fn missing_image_buf() -> SharedPixelBuffer<Rgba8Pixel> {
    let bytes = include_bytes!("../ui/assets/missing-image.png");
    let image = image::load_from_memory(bytes).unwrap();
    let image = image
        .resize(440 as u32, 248 as u32, image::imageops::FilterType::Nearest)
        .into_rgba8();
    SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(image.as_raw(), image.width(), image.height())
}

pub fn _missing_image() -> slint::Image {
    slint::Image::from_rgba8(missing_image_buf())
}

pub fn create_deets_window(
    item: GbPreviewData,
    weak: Weak<App>,
    dark_rx: broadcast::Receiver<ColorScheme>,
) -> GbDetailsWindow {
    let deets = GbDetailsWindow::new().unwrap();
    if let Ok(cfg) = R4D_CFG.try_lock() {
        deets.invoke_set_color_scheme(if cfg.dark_mode {
            ColorScheme::Dark
        } else {
            ColorScheme::Light
        });
    }
    let item_id = item.id.clone();

    deets
        .global::<HyperLink>()
        .on_open_hyperlink(|link| match open::that(link.to_string()) {
            Ok(_) => {}
            Err(e) => eprintln!("{e}"),
        });

    let deets_weak = deets.as_weak();
    if !item.image_loaded && !item.image_url.is_empty() {
        let url = item.image_url.to_string();
        println!("Loading image for preview window: {}", url);
        tokio::spawn(async move {
            let buf = match get_image(url).await {
                Ok(buf) => buf,
                Err(e) => {
                    eprintln!("{e}");
                    return;
                }
            };
            println!("Got image");
            let _ = deets_weak.upgrade_in_event_loop(move |deets| {
                let mut data = deets.get_data();
                data.image = slint::Image::from_rgba8(buf);
                data.image_loaded = true;
                deets.set_data(data);
            });
        });
    }
    deets.set_data(item);
    let deets_weak = deets.as_weak();

    tokio::spawn(async move {
        match fetch_mod_info(item_id).await {
            Ok(module) => {
                let _ = deets_weak.upgrade_in_event_loop(move |deets| {
                    let vecmod: VecModel<Download> = VecModel::default();
                    for file in module.files.unwrap_or(vec![]) {
                        vecmod.push(file.into());
                    }
                    deets.set_files(ModelRc::new(vecmod));
                    deets.set_description(
                        module.text.unwrap_or_default().replace("<br>", "\n").into(),
                    );
                });
            }
            Err(e) => open_error_window(e.to_string()),
        }
    });

    let weak = weak.clone();
    let deets_weak = deets.as_weak();
    deets
        .global::<GameBananaLogic>()
        .on_download(move |download| {
            let weak = weak.clone();
            println!("{}", download.url.to_string());
            let deets = deets_weak.unwrap();
            let model = deets.get_files();
            let files = match model.as_any().downcast_ref::<VecModel<Download>>() {
                Some(vec) => vec,
                None => return,
            };
            if let Some(idx) = files.iter().position(|i| i.id == download.id) {
                let deets_weak = deets_weak.clone();
                let (tx, mut rx) = channel::<usize>(30000);
                let row = idx.clone();
                tokio::spawn(async move {
                    let wait_time = tokio::time::Duration::from_millis(50);
                    while !rx.is_closed() || !rx.is_empty() {
                        if let Ok(len) = rx.try_recv() {
                            let row = row.clone();
                            let _ = deets_weak.upgrade_in_event_loop(move |deets| {
                                if let Some(mut dl) = deets.get_files().row_data(row) {
                                    dl.progress += len as i32;
                                    deets.get_files().set_row_data(row, dl);
                                }
                            });
                        } else {
                            sleep(wait_time).await;
                        }
                    }
                });

                tokio::spawn(async move {
                    let req = reqwest_client().get(download.url.to_string()).send();
                    let res = match req.await {
                        Ok(res) => match res.error_for_status() {
                            Ok(res) => res,
                            Err(e) => {
                                open_error_window(e.to_string());
                                return;
                            }
                        },
                        Err(e) => {
                            open_error_window(e.to_string());
                            return;
                        }
                    };
                    println!("{}", res.status());
                    let mut stream = res.bytes_stream();
                    let mut bytes = vec![];
                    let tx = tx;
                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(chunk) => {
                                let _ = tx.try_send(chunk.len());
                                bytes.push(chunk);
                            }
                            Err(e) => {
                                open_error_window(e.to_string());
                                return;
                            }
                        }
                    }
                    println!("Done, len: {}", bytes.len());
                    if let Some(dir) = get_temp_folder() {
                        let mut buf = PathBuf::from(dir);
                        buf.push(download.name.to_string());
                        match File::create(buf.clone()) {
                            Ok(mut file) => {
                                for chunk in bytes {
                                    if let Err(e) = file.write_all(&chunk) {
                                        open_error_window(e.to_string());
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                open_error_window(e.to_string());
                                return;
                            }
                        }
                        match unpack_mod_path(buf).await {
                            Ok(_) => {
                                if load_mods().is_ok() {
                                    match set_mods_table(&get_mods(), weak.clone()) {
                                        Ok(_) => {}
                                        Err(e) => eprintln!("{e}"),
                                    }
                                }
                            }
                            Err(e) => {
                                open_error_window(e.to_string());
                            }
                        }
                    }
                });
            }
        });

    let deets_weak = deets.as_weak();
    let mut scheme_rx = dark_rx.resubscribe();
    let scheme_changer = tokio::spawn(async move {
        while let Ok(scheme) = scheme_rx.recv().await {
            let _ = deets_weak.upgrade_in_event_loop(move |deets| {
                deets.invoke_set_color_scheme(scheme);
            });
        }
    });

    deets.window().on_close_requested(move || {
        scheme_changer.abort();
        slint::CloseRequestResponse::HideWindow
    });
    deets
}

pub async fn fetch_mod(id: i32) -> Result<GBSearch, Box<dyn Error + Send + Sync>> {
    let res = reqwest_client().get(get_mod_url(id)).send().await?;
    let text = res.text().await?;
    match sonic_rs::from_str::<GBSearch>(&text) {
        Ok(search) => Ok(search),
        Err(e) => {
            eprintln!("{text}");
            Err(e.into())
        }
    }
}

pub fn get_mod_url(id: i32) -> String {
    format!("{GB_DOMAIN}/{GB_MOD_DATA}/{id}/ProfilePage")
}
