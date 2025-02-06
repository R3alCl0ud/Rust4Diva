use std::error::Error;

use serde::{Deserialize, Serialize};

use crate::util::reqwest_client;

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

#[derive(Serialize, Deserialize)]
pub struct Post {
    pub id: i32,
    pub name: String,
    pub text: String,
    pub images: Vec<String>,
    pub file: String,
    pub time: i64,
    pub post_type: PostType,
    pub download_count: i64,
    pub like_count: i64,
    pub authors: Vec<User>,
    pub dependencies: Option<Vec<Post>>,
}

#[derive(Clone)]
pub struct Comment {
    pub id: i32,
    pub user: User,
    pub text: String,
}

pub async fn search(query: String) -> Result<Vec<Post>, Box<dyn Error>> {

    let mut client = reqwest_client();
    let mut req = client.get(format!("{}/posts", DMA_DOMAIN)).query(&["query", &query]);
    let res = req.send().await?.text().await?;

    match sonic_rs::from_str::<Vec<Post>>(&res) {
        Ok(posts) => Ok(posts),
        Err(e) => {
            eprintln!("{}", res); // log the res that failed to parse
            Err(e.into())
        }

    }
}
