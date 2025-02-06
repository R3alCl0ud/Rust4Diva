use std::{fs::File, io::Write, path::PathBuf};

use futures_util::StreamExt;
use slint::{ComponentHandle, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel, Weak};
use tokio::{
    sync::{broadcast, mpsc::channel},
    time::sleep,
};

use crate::{
    diva::{get_temp_folder, open_error_window},
    modmanagement::{get_mods, load_mods, set_mods_table, unpack_mod_path},
    util::reqwest_client,
    App, Download, GameBananaLogic, HyperLink, SearchDetailsWindow, SearchPreviewData, R4D_CFG,
};
use slint::private_unstable_api::re_exports::ColorScheme;

pub async fn init(app: &App) {}

pub fn missing_image_buf() -> SharedPixelBuffer<Rgba8Pixel> {
    let bytes = include_bytes!("../ui/assets/missing-image.png");
    let image = image::load_from_memory(bytes).unwrap();
    let image = image
        .resize(440 as u32, 248 as u32, image::imageops::FilterType::Nearest)
        .into_rgba8();
    SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(image.as_raw(), image.width(), image.height())
}

pub fn create_deets_window(
    item: SearchPreviewData,
    weak: Weak<App>,
    dark_rx: broadcast::Receiver<ColorScheme>,
) -> SearchDetailsWindow {
    let deets = SearchDetailsWindow::new().unwrap();
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
            let buf = match crate::gamebanana::get_image(url).await {
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
        match crate::gamebanana::fetch_mod_info(item_id).await {
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
