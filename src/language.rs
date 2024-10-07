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
    pub enum Langs(#[constructor]as_code: &'static str, to_string: &'static str,  default_dict: &[u8]) {
        EnUS("enUS", "English", include_bytes!("lang/enUS.lang")),
        EnLT("enLT", "1337", include_bytes!("lang/enLEET.lang")),
        JaJP("jaJP", "日本語", include_bytes!("lang/jaJP.lang")),
        EsPR("esPR", "español", include_bytes!("lang/esPR.lang")),
        ZhCN("zhCN", "简体中文", include_bytes!("lang/zhCN.lang"))
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
            Langs::EnLT,
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
            1337 => Langs::EnLT,
            _ => Langs::EnUS,
        }
    }
}

pub static TRANSLATIONS: LazyLock<Mutex<HashMap<Langs, HashMap<String, String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub async fn load_translations() -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut lang_library = get_config_dir()?;

    lang_library.push("langs");
    if !lang_library.exists() {
        fs::create_dir_all(lang_library.clone())?;
    }

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
        let mut dictionary = parse_lang(String::from_utf8(lang.default_dict().to_vec())?);

        let mut dict_path = lang_library.clone();
        dict_path.push(format!("{}.lang", lang.as_code()));
        let content = match dict_path.exists() {
            true => match fs::read_to_string(dict_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("{e}");
                    "".to_owned()
                }
            },
            false => "".to_owned(),
        };
        println!("Parsing: {lang}");
        for (key, definition) in parse_lang(content) {
            dictionary.insert(key, definition);
        }
        langs.insert(lang.clone(), dictionary);
    }
    #[cfg(debug_assertions)]
    println!("Loaded Translations");

    Ok(())
}

pub async fn init_ui(app: &App) {
    match load_translations().await {
        Ok(_) => {}
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
