use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use slint::{ComponentHandle, SharedString};

use crate::slint_generatedApp::App;
use crate::{LangTL, R4D_CFG};

table_enum::table_enum! {
    #[derive(Hash, Eq, PartialEq)]
    pub enum Langs(#[constructor] to_string: &'static str) {
        EnUS("English"),
        JaJP("日本語"),
        EsPR("español")
    }

}

impl From<i32> for Langs {
    fn from(value: i32) -> Self {
        match value {
            2 => Langs::JaJP,
            3 => Langs::EsPR,
            _ => Langs::EnUS,
        }
    }
}

pub static TRANSLATIONS: LazyLock<Mutex<HashMap<Langs, HashMap<String, String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const ENUS: &[u8] = include_bytes!("lang/enUS.lang");
const JAJP: &[u8] = include_bytes!("lang/jaJP.lang");
const ESPR: &[u8] = include_bytes!("lang/esPR.lang");

pub async fn init_ui(app: &App) {
    let en_us: HashMap<String, String> =
        parse_lang(String::from_utf8(ENUS.to_vec()).unwrap_or("".to_owned()));
    let ja_jp: HashMap<String, String> =
        parse_lang(String::from_utf8(JAJP.to_vec()).unwrap_or("".to_owned()));
    let es_pr: HashMap<String, String> =
        parse_lang(String::from_utf8(ESPR.to_vec()).unwrap_or("".to_owned()));

    if let Ok(mut langs) = TRANSLATIONS.lock() {
        langs.insert(Langs::EnUS, en_us);
        langs.insert(Langs::JaJP, ja_jp);
        langs.insert(Langs::EsPR, es_pr);
        #[cfg(debug_assertions)]
        println!("Loaded Translations");
    }

    app.global::<LangTL>()
        .on_get_localized_string(move |unlocalized| {
            let lang = match R4D_CFG.try_lock() {
                Ok(cfg) => Langs::from(cfg.lang),
                Err(_) => todo!(),
            };
            let tl = match TRANSLATIONS.lock() {
                Ok(tls) => {
                    if let Some(tl) = tls.get(&lang) {
                        tl.clone()
                    } else {
                        return unlocalized;
                    }
                }
                Err(_) => return unlocalized,
            };
            let localized: SharedString = match tl.get(&unlocalized.to_string()) {
                Some(localized) => localized.into(),
                None => unlocalized,
            };
            return localized;
        });
}

pub fn parse_lang(lang: String) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let lines = lang.lines();
    for line in lines {
        if let Some((key, content)) = line.split_once("=") {
            println!("{line}");
            map.insert(key.to_owned(), content.to_owned());
        }
    }

    map
}
