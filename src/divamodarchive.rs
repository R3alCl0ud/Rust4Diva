use std::sync::mpsc::Receiver;
use std::{cmp::min, error::Error};

use crate::diva::open_error_window;
use crate::downloads::{get_image, missing_image_buf};
use crate::SearchProvider;
use crate::{util::reqwest_client, App, DMALogic, Download, SearchModAuthor, SearchPreviewData};
use regex::Regex;
use serde::{Deserialize, Serialize};
use slint::private_unstable_api::re_exports::ColorScheme;
use slint::{ComponentHandle, Model, ModelRc, SharedString, ToSharedString, VecModel, Weak};
use tokio::sync::broadcast::{self};
pub const DMA_DOMAIN: &str = "https://divamodarchive.com";

#[repr(i32)]
#[derive(PartialEq, Serialize, Deserialize, Clone)]
pub enum PostType {
    Plugin = 0,
    Module = 1,
    Song = 2,
    Cover = 3,
    Ui = 4,
    Other = 5,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub avatar: String,
    pub display_name: String,
}

impl From<i32> for PostType {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Plugin,
            1 => Self::Module,
            2 => Self::Song,
            3 => Self::Cover,
            4 => Self::Ui,
            _ => Self::Other,
        }
    }
}

impl std::fmt::Display for PostType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PostType::Plugin => "Plugin",
            PostType::Module => "Module",
            PostType::Song => "Song",
            PostType::Cover => "Cover",
            PostType::Ui => "UI",
            PostType::Other => "Other",
        })
    }
}

impl From<Post> for SearchPreviewData {
    fn from(value: Post) -> Self {
        let author: SearchModAuthor = match value.authors.first() {
            Some(user) => user.clone().into(),
            None => SearchModAuthor {
                avatar_url: "sadjim.png".into(),
                name: "Unknown".into(),
            },
        };

        let img_url: SharedString = match value.images.first() {
            Some(url) => url.into(),
            None => "".into(),
        };
        let mut files = vec![];
        for i in 0..min(value.files.len(), value.file_names.len()) {
            files.push(Download {
                failed: false,
                id: (value.id as i64 | ((i as i64) << 32)).to_shared_string(),
                inprogress: false,
                name: value.file_names[i].clone().into(),
                progress: 0,
                size: 0,
                url: value.files[i].clone().into(),
                provider: SearchProvider::DivaModArchive,
            });
        }
        Self {
            author: author,
            files: ModelRc::new(VecModel::from(files)),
            id: value.id,
            image: Default::default(),
            image_loaded: false,
            image_url: img_url,
            item_type: value.post_type.to_shared_string(),
            name: value.name.into(),
            submitted: value.time.clone().into(),
            updated: value.time.clone().into(),
            description: value.text.clone().into(),
            provider: SearchProvider::DivaModArchive,
        }
    }
}

impl From<User> for SearchModAuthor {
    fn from(value: User) -> Self {
        Self {
            avatar_url: value.avatar.into(),
            name: value.display_name.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Post {
    pub id: i32,
    pub name: String,
    pub text: String,
    pub images: Vec<String>,
    pub files: Vec<String>,
    pub time: String,
    pub post_type: PostType,
    pub download_count: i64,
    pub like_count: i64,
    pub authors: Vec<User>,
    pub dependencies: Option<Vec<Post>>,
    pub file_names: Vec<String>,
}

#[derive(Clone)]
pub struct _Comment {
    pub id: i32,
    pub user: User,
    pub text: String,
}

pub async fn init(ui: &App, _dark_rx: broadcast::Receiver<ColorScheme>) {
    let ui_search_handle = ui.as_weak();
    ui.global::<DMALogic>()
        .on_search(move |term, _page, _sort| {
            println!("{}", term);
            let ui_search_handle = ui_search_handle.clone();
            let ui_result_handle = ui_search_handle.clone();
            ui_search_handle.unwrap().set_s_prog_vis(true);
            tokio::spawn(async move {
                match search(term.to_string()).await {
                    Ok(posts) => ui_search_handle.upgrade_in_event_loop(move |ui| {
                        let res_vec_model: VecModel<SearchPreviewData> = VecModel::default();
                        let posts_len = posts.len();
                        for post in posts.iter() {
                            res_vec_model.push(post.clone().into());
                        }
                        ui.set_s_results(ModelRc::new(res_vec_model));
                        ui.set_n_results(posts_len as i32);
                        ui.set_s_prog_vis(false);
                        for post in posts {
                            let weak = ui_result_handle.clone();
                            let post = post.clone();
                            tokio::spawn(async move {
                                get_and_set_preview_image(weak, post).await;
                            });
                        }
                    }),
                    Err(err) => {
                        open_error_window(err.to_string());
                        Ok(())
                    }
                }
            });
        });
}

pub async fn search(query: String) -> Result<Vec<Post>, Box<dyn Error>> {
    let client = reqwest_client();
    let req = client
        .get(format!("{}/api/v1/posts", DMA_DOMAIN))
        .query(&[("query", &query), ("limit", &"2000".to_owned())]);
    let res = req.send().await?.text().await?;

    match sonic_rs::from_str::<Vec<Post>>(&res) {
        Ok(posts) => Ok(posts),
        Err(e) => {
            eprintln!("dma.rs @ search(query) : {}", res); // log the res that failed to parse
            Err(e.into())
        }
    }
}

pub async fn get_and_set_preview_image(weak: Weak<App>, item: Post) {
    let mut buffer = missing_image_buf();
    if !item.images.is_empty() {
        if let Ok(buf) = get_image(item.images.first().unwrap().clone()).await {
            buffer = buf;
        }
    }
    let _ = weak.upgrade_in_event_loop(move |ui| {
        let image = slint::Image::from_rgba8(buffer);
        let model = ui.get_s_results();
        let results = match model.as_any().downcast_ref::<VecModel<SearchPreviewData>>() {
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

pub async fn fetch_post(id: String) -> Result<Post, Box<dyn Error + Sync + Send>> {
    let client = reqwest_client();
    let builder = client.get(format!("{}/api/v1/posts/{}", DMA_DOMAIN, id));
    let res = builder.send().await?;
    let text = res.text().await?;

    match sonic_rs::from_str::<Post>(&text) {
        Ok(post) => Ok(post),
        Err(e) => {
            eprintln!("dma.rs @ fetch_post(id) -> {}", text); // log the res that failed to parse
            Err(e.into())
        }
    }
}

pub fn parse_oneclick_url(url: String) -> Option<String> {
    let regex = Regex::new(r"divamodmanager:dma/(\d+)").unwrap();

    let Some(m_info) = regex.captures(url.as_str()) else {
        return None;
    };
    return Some(m_info.get(1).unwrap().as_str().to_string());
}
