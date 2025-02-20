use std::{cmp::min, error::Error};

use serde::{Deserialize, Serialize};
use slint::private_unstable_api::re_exports::ColorScheme;
use slint::{ComponentHandle, ModelRc, SharedString, ToSharedString, VecModel};
use tokio::sync::{broadcast, mpsc::Receiver};

use crate::diva::open_error_window;
use crate::gamebanana::GBSearch;
use crate::{util::reqwest_client, App, DMALogic, Download, SearchModAuthor, SearchPreviewData};
pub const DMA_DOMAIN: &str = "https://divamodarchive.com";

#[repr(i32)]
#[derive(PartialEq, Serialize, Deserialize)]
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
                id: value.id,
                inprogress: false,
                name: value.file_names[i].clone().into(),
                progress: 0,
                size: 0,
                url: value.files[i].clone().into(),
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
            preloaded: true,
            submitted: value.time.clone().into(),
            updated: value.time.clone().into(),
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

#[derive(Serialize, Deserialize)]
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
pub struct Comment {
    pub id: i32,
    pub user: User,
    pub text: String,
}

pub async fn init(ui: &App, dark_rx: broadcast::Receiver<ColorScheme>) {
    let ui_search_handle = ui.as_weak();
    ui.global::<DMALogic>().on_search(move |term, page, sort| {
        println!("{}", term);
        let ui_search_handle = ui_search_handle.clone();
        tokio::spawn(async move {
            match search(term.to_string()).await {
                Ok(posts) => ui_search_handle.upgrade_in_event_loop(move |ui| {
                    let res_vec_model: VecModel<SearchPreviewData> = VecModel::default();
                    for post in posts {
                        res_vec_model.push(post.into());
                    }
                    ui.set_s_results(ModelRc::new(res_vec_model));
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
    let mut client = reqwest_client();
    let mut req = client
        .get(format!("{}/api/v1/posts", DMA_DOMAIN))
        .query(&[("query", &query)]);
    let res = req.send().await?.text().await?;

    match sonic_rs::from_str::<Vec<Post>>(&res) {
        Ok(posts) => Ok(posts),
        Err(e) => {
            eprintln!("dma.rs @ search(query) : {}", res); // log the res that failed to parse
            Err(e.into())
        }
    }
}
