use std::collections::HashMap;
use std::error::Error;
use std::fmt::Display;
use std::slice::Iter;
use std::sync::{LazyLock, Mutex};
use std::{fs, io};

use slint::{ComponentHandle, SharedString};

use crate::diva::get_config_dir;
use crate::slint_generatedApp::App;
use crate::{LangTL, R4D_CFG};

table_enum::table_enum! {
    #[derive(Hash, Eq, PartialEq, Clone)]
    pub enum Langs(#[constructor] to_string: &'static str, file_name: &'static str) {
        EnUS("English", "enUS"),
        EnLEET("1337", "enLEET"),
        JaJP("日本語", "jaJP"),
        EsPR("español", "esPR"),
        ZhCN("简体中文", "zhCN")
    }
}

impl Display for Langs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl Langs {
    pub fn iter() -> Iter<'static, Langs> {
        static LANGS: [Langs; 5] = [
            Langs::EnUS,
            Langs::EnLEET,
            Langs::JaJP,
            Langs::EsPR,
            Langs::ZhCN,
        ];
        return LANGS.iter();
    }
}

impl From<i32> for Langs {
    fn from(value: i32) -> Self {
        match value {
            1 => Langs::JaJP,
            2 => Langs::EsPR,
            3 => Langs::ZhCN,
            1337 => Langs::EnLEET,
            _ => Langs::EnUS,
        }
    }
}

pub static TRANSLATIONS: LazyLock<Mutex<HashMap<Langs, HashMap<String, String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const ENUS_DEFAULT: &[u8] = include_bytes!("lang/enUS.lang");
const LEET_DEFAULT: &[u8] = include_bytes!("lang/enLEET.lang");
const JAJP_DEFAULT: &[u8] = include_bytes!("lang/jaJP.lang");
const ESPR_DEFAULT: &[u8] = include_bytes!("lang/esPR.lang");
const ZHCN_DEFAULT: &[u8] = include_bytes!("lang/zhCN.lang");

pub async fn load_translations() -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut lang_library = get_config_dir()?;

    lang_library.push("langs");
    if !lang_library.exists() {
        fs::create_dir_all(lang_library.clone())?;
    }

    // Langs::
    let mut langs = match TRANSLATIONS.lock() {
        Ok(langs) => langs,
        Err(_) => {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Failed to acquire lock",
            )));
        }
    };
    for lang in Langs::iter() {
        let mut dict_path = lang_library.clone();
        dict_path.push(format!("{lang}.lang"));
        let content = match dict_path.exists() {
            true => match fs::read_to_string(dict_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("{e}");
                    "".to_owned()
                }
            },
            false => match lang {
                Langs::EnUS => String::from_utf8(ENUS_DEFAULT.to_vec()).unwrap_or("".to_owned()),
                Langs::EnLEET => String::from_utf8(LEET_DEFAULT.to_vec()).unwrap_or("".to_owned()),
                Langs::JaJP => String::from_utf8(JAJP_DEFAULT.to_vec()).unwrap_or("".to_owned()),
                Langs::EsPR => String::from_utf8(ESPR_DEFAULT.to_vec()).unwrap_or("".to_owned()),
                Langs::ZhCN => String::from_utf8(ZHCN_DEFAULT.to_vec()).unwrap_or("".to_owned()),
            },
        };
        println!("Parsing: {lang}");
        langs.insert(lang.clone(), parse_lang(content));
    }
    #[cfg(debug_assertions)]
    println!("Loaded Translations");

    Ok(())
}

pub async fn init_ui(app: &App) {

    match load_translations().await {
        Ok(_) => {},
        Err(e) => eprintln!("{e}"),
    }

    app.global::<LangTL>()
        .on_get_localized_string(move |unlocalized| {
            let language = match R4D_CFG.try_lock() {
                Ok(cfg) => Langs::from(cfg.lang),
                Err(_) => todo!(),
            };
            let translations = match TRANSLATIONS.lock() {
                Ok(tls) => tls,
                Err(_) => todo!(),
            };

            let dictionary = translations
                .get(&language)
                .expect("Should have returned a valid dictionary");

            let localized: SharedString = match dictionary.get(&unlocalized.to_string()) {
                Some(localized) => localized.into(),
                None => translations
                    .get(&Langs::EnUS)
                    .unwrap()
                    .get(&unlocalized.to_string())
                    .unwrap_or(&unlocalized.to_string())
                    .into(),
            };
            return localized;
        });
}

pub fn parse_lang(lang: String) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let lines = lang.lines();
    for line in lines {
        if let Some((key, content)) = line.split_once("=") {
            // println!("{line}");
            map.insert(key.to_owned(), content.to_owned());
        }
    }

    map
}
