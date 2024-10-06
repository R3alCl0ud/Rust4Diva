use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use slint::{ComponentHandle, SharedString};

use crate::slint_generatedApp::App;
use crate::{LangTL, R4D_CFG};

#[derive(Hash, Clone, Debug)]
enum Langs {
    EnUS,
    JaJP,
}

impl Langs {
    pub fn to_string(self: &Self) -> String {
        String::from(self)
    }
}

impl Default for Langs {
    fn default() -> Self {
        Langs::EnUS
    }
}

impl From<Langs> for String {
    fn from(value: Langs) -> Self {
        match value {
            Langs::EnUS => "English".to_string(),
            Langs::JaJP => "日本語".to_string(),
        }
    }
}

impl From<&Langs> for String {
    fn from(value: &Langs) -> Self {
        match value {
            Langs::EnUS => "English".to_string(),
            Langs::JaJP => "日本語".to_string(),
        }
    }
}

impl From<i32> for Langs {
    fn from(value: i32) -> Self {
        match value {
            2 => Langs::JaJP,
            _ => Langs::EnUS,
        }
    }
}

pub static TRANSLATIONS: LazyLock<Mutex<HashMap<String, HashMap<String, String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const ENUS: &[u8] = include_bytes!("lang/enUS.lang");
const JAJP: &[u8] = include_bytes!("lang/jaJP.lang");

pub async fn init_ui(app: &App) {
    let en_us: HashMap<String, String> =
        parse_lang(String::from_utf8(ENUS.to_vec()).unwrap_or("".to_owned()));
    let ja_jp: HashMap<String, String> =
        parse_lang(String::from_utf8(JAJP.to_vec()).unwrap_or("".to_owned()));

    if let Ok(mut langs) = TRANSLATIONS.lock() {
        langs.insert(Langs::EnUS.to_string(), en_us);
        langs.insert(Langs::JaJP.to_string(), ja_jp);
        #[cfg(debug_assertions)]
        println!("Loaded Translations");
    }

    app.global::<LangTL>()
        .on_get_localized_string(move |unlocalized| {
            let lang = match R4D_CFG.try_lock() {
                Ok(cfg) => Langs::from(cfg.lang).to_string(),
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
