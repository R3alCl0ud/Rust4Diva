use core::time::Duration;
use std::{env, io};
use std::cmp::PartialEq;
use std::error::Error;
use std::future::Future;
use std::sync::{Arc, mpsc, Mutex};
use std::thread::JoinHandle;

use eframe::{CreationContext, egui, Frame};
use eframe::emath::Align;
use egui::{Color32, Context, DroppedFile, FontId, Layout, ProgressBar, Style, TextBuffer, TextStyle, vec2, Visuals};
use std::collections::HashMap;
use egui::Align::{Center, Min};
use egui::FontFamily::{Monospace, Proportional};
use egui_extras::{Column, TableBuilder};
use interprocess::local_socket::{
    GenericFilePath,
    GenericNamespaced, tokio::{prelude::*, Stream},
};
use interprocess::local_socket::ListenerOptions;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::try_join;

use crate::gamebanana_async::{download_mod_file, fetch_mod_data, GBMod, GbModDownload, reqwest_mod_data};
use crate::modmanagement::{create_tmp_if_not, DivaMod, DivaModLoader, get_diva_folder, load_diva_ml_config, load_mods, save_mod_config, save_mod_configs, unpack_mod};

mod modmanagement;
mod gamebanana_async;


const CYCLE_TIME: Duration = Duration::from_secs(1);


struct DivaData {
    mods: Vec<DivaMod>,
    current_dl: Option<GBMod>,
    downloads: Vec<GBMod>,
    mods_directory: String,
    diva_directory: String,
    dl_mod_url: String,
    loaded: bool,
    dropped_files: Vec<DroppedFile>,
    show_dl: bool,
    dml: DivaModLoader,
    dlthreads: Vec<(JoinHandle<()>, mpsc::SyncSender<egui::Context>)>,
    dl_done_tx: Sender<GbModDownload>,
    dl_done_rc: Receiver<GbModDownload>,
    should_dl: bool,
    dmm_url_rx: Receiver<String>,
    dl_progress: HashMap<String, Receiver<u64>>,
}

struct ModDlThread {
    name: String,
    // gbmod: GbModDownload,
    progress: Arc<Mutex<f32>>,
}

impl ModDlThread {
    // , gbmod: GbModDownload
    fn new(name: String) -> Self {
        Self {
            name,
            // gbmod,
            progress: Arc::new(Mutex::new(0.0)),
        }
    }

    fn show(&mut self, ctx: &egui::Context) {
        egui::Window::new("&self.gbmod._sFile")
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Downloading mod");
                    let progress = ProgressBar::new(*self.progress.lock().unwrap())
                        .text("&self.gbmod._sFile");
                    ui.add(progress);
                });
            });
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init(); // let us println! all the things!
    println!("Starting Rust4Diva");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 720.0]),
        ..Default::default()
    };

    let args = env::args().collect::<Vec<String>>();

    let mut dl_mod = "".to_string();

    for arg in args.iter() {
        if arg.starts_with("divamodmanager:") {
            println!("Found a mod to dl passed in as an arg {:#}", arg);
            dl_mod = arg.to_string();
            break;
        }
    }

    println!("{}", env::consts::OS);

    match create_tmp_if_not() {
        Ok(..) => {}
        Err(e) => {
            eprintln!("Unable to create my temp directory, {}", e);
        }
    }

    // if !dl_mod.is_empty() {
    //     return Ok(());
    // }

    let (url_tx, mut rx) = tokio::sync::mpsc::channel(2048);
    // rx.try_recv();
    match spawn_listener(url_tx).await {
        Ok(listening) => {
            if listening {
                match eframe::run_native(
                    "Rust4Diva",
                    options,
                    Box::new(|cc| {
                        let style = Style {
                            visuals: Visuals::light(),
                            ..Style::default()
                        };
                        cc.egui_ctx.set_style(style);
                        Ok(Box::new(DivaData::new(cc, rx)))
                    }),
                ) {
                    Ok(..) => Ok(()),
                    Err(e) => Err(e.into())
                }
            } else {
                Ok(())
            }
        }
        Err(e) => return Err(e)
    }
}


// #[tokio::main]
/// This is the function for the url handling, should this return Result(True) we know that we are
/// the listening server and should run the display window
async fn spawn_listener(dmm_url_tx: Sender<String>) -> Result<bool, Box<dyn std::error::Error>> {
    println!("Starting dmm url listener");


    let printname = "rust4diva.sock";
    // Pick a name.
    let name = if GenericNamespaced::is_supported() {
        printname.to_ns_name::<GenericNamespaced>()?
    } else {
        format!("/tmp/{}", printname).to_fs_name::<GenericFilePath>()?
    };

    // Await this here since we can't do a whole lot without a connection.
    // let conn = Stream::connect(name).await;

    // let send_url = dmm_url_tx.clone();
    async fn handle_conn(conn: Stream, send_url: Sender<String>) -> io::Result<()> {
        let mut recver = BufReader::new(&conn);
        let mut sender = &conn;

        // Allocate a sizeable buffer for receiving. This size should be big enough and easy to
        // find for the allocator.
        let mut buffer = String::with_capacity(128);

        // Describe the send operation as sending our whole message.
        let send = sender.write_all(b"URL Recieved\n");
        // Describe the receive operation as receiving a line into our big buffer.
        let recv = recver.read_line(&mut buffer);

        // Run both operations concurrently.
        try_join!(recv, send)?;

        // Produce our output!
        println!("DMM Url: {}", buffer.trim());
        // let dmm_str = buffer.trim().clone().to_owned();
        let mut dmm_url = buffer.trim();
        // dmm_url_tx
        send_url.send(dmm_url.to_string()).await.expect("unable to transmit the received url to main thread");

        Ok(())
    }


    // let name = printname.to_ns_name::<GenericNamespaced>()?;

    // Configure our listener...
    let opts = ListenerOptions::new().name(name);

    // ...and create it.
    let listener = match opts.create_tokio() {
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            eprintln!(
                "
Error: could not start server because the socket file is occupied. Please check if {printname}
is in use by another process and try again."
            );
            return Err(e.into());
        }
        x => x?,
    };

    // The syncronization between the server and client, if any is used, goes here.
    eprintln!("Server running at {printname}");
    // Set up our loop boilerplate that processes our incoming connections.

    tokio::spawn(async move {
        loop {
            let url_tx: Sender<String> = dmm_url_tx.clone();
            // Sort out situations when establishing an incoming connection caused an error.
            let conn = match listener.accept().await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("There was an error with an incoming connection: {e}");
                    continue;
                }
            };
            // Spawn new parallel asynchronous tasks onto the Tokio runtime
            // tokio::spawn(async move {
            // The outer match processes errors that happen when we're connecting to something.
            // The inner if-let processes errors that happen during the connection.
            if let Err(e) = handle_conn(conn, url_tx).await {
                eprintln!("Error while handling connection: {e}");
            }
            // });
        }
    });
    Ok(true)
}

fn start_ipc(name: String, dl_done_tx: Sender<GbModDownload>) -> (JoinHandle<()>, mpsc::SyncSender<egui::Context>) {
    let (show_tx, show_rc) = mpsc::sync_channel(0);
    let handle = std::thread::Builder::new().name(name.clone())
        .spawn(move || {
            let mut state = ModDlThread::new(name);
            while let Ok(ctx) = show_rc.recv() {
                state.show(&ctx);
                // let _ = dl_done_tx.send(0.01);
            }
        }).expect("fuck");
    (handle, show_tx)
}


impl DivaData {
    fn new(_cc: &CreationContext, dmm_rx: Receiver<String>) -> Self {
        let (dl_tx, mut dl_rx) = tokio::sync::mpsc::channel::<GbModDownload>(2048);
        // let (dl_done_tx, dl_done_rc) = mpsc::sync_channel::(0);
        let mut slf = Self {
            mods: Vec::new(),
            downloads: Vec::new(),
            current_dl: None,
            loaded: false,
            mods_directory: "".to_string(),
            dropped_files: Vec::new(),
            dl_mod_url: "".to_string(),
            show_dl: false,
            diva_directory: "".to_string(),
            dml: DivaModLoader {
                enabled: false,
                console: false,
                mods: "".to_string(),
                version: "".to_string(),
            },
            dlthreads: Vec::with_capacity(1),
            dl_done_tx: dl_tx,
            dl_done_rc: dl_rx,
            should_dl: false,
            dmm_url_rx: dmm_rx,
            dl_progress: HashMap::new(),
        };

        // slf.spawn_dl_thread();

        slf
    }

    fn spawn_dl_thread(&mut self) {
        self.dlthreads.push(start_ipc("test".to_owned(), self.dl_done_tx.clone()));
    }
}

impl std::ops::Drop for DivaData {
    fn drop(&mut self) {
        for (handle, show_tx) in self.dlthreads.drain(..) {
            std::mem::drop(show_tx);
            handle.join().unwrap();
        }
    }
}

impl PartialEq for DivaMod {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl eframe::App for DivaData {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        if !self.loaded {
            // try to find the mods folder
            self.diva_directory = get_diva_folder().expect("Shit, something no worky");

            // load diva mod loader config
            self.dml = load_diva_ml_config(&self.diva_directory.as_str()).unwrap();
            // load the mod folder now that we've hopefully found it
            self.mods = load_mods(self);
            // set loaded so this doesn't get called 20 billion times.
            self.loaded = true;
        }
        if let Ok(dmm_url) = self.dmm_url_rx.try_recv() {
            println!("recieved dmm url on gui thread: {}", dmm_url);
        }

        if let Ok(gb_mod_dl) = self.dl_done_rc.try_recv() {
            println!("Finished downloading: {}", &gb_mod_dl._sFile);
            unpack_mod(&gb_mod_dl, &self);
            println!("Reloading mods");
            self.mods = load_mods(self);
        }
        // begin the top bar of the app
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                ui.label("File");
                ui.label("Edit");
                ui.label("View");
                ui.label("Help");
            });
        });

        let side_frame = egui::containers::Frame {
            inner_margin: Default::default(),
            rounding: Default::default(),
            shadow: Default::default(),
            fill: Color32::LIGHT_BLUE,
            stroke: egui::Stroke::new(0.0, Color32::GOLD),
            outer_margin: Default::default(),
        };
        egui::SidePanel::left("dml-info").frame(side_frame).show(ctx, |ui| {
            ui.heading("Diva Mod Loader");
            let text = if self.dml.enabled { "Enabled" } else { "Disabled" };
            if ui.checkbox(&mut self.dml.enabled, text).clicked() {}
            ui.label(format!("Version: {}", &self.dml.version));
        });

        let center_frame = egui::containers::Frame {
            inner_margin: Default::default(),
            rounding: Default::default(),
            shadow: Default::default(),
            fill: Color32::LIGHT_GRAY,
            stroke: egui::Stroke::new(0.0, Color32::GOLD),
            outer_margin: Default::default(),
        };

        // begin the actual contents
        egui::CentralPanel::default().frame(center_frame).show(ctx, |ui| {
            ui.style_mut().spacing.item_spacing = vec2(16.0, 16.0);
            let mut style = (*ctx.style()).clone();
            style.text_styles = [
                (TextStyle::Heading, FontId::new(25.0, Proportional)),
                (TextStyle::Body, FontId::new(24.0, Proportional)),
                (TextStyle::Monospace, FontId::new(24.0, Monospace)),
                (TextStyle::Button, FontId::new(24.0, Proportional)),
                (TextStyle::Small, FontId::new(16.0, Proportional)),
            ].into();
            ctx.set_style(style);
            ui.with_layout(Layout::top_down_justified(Align::Center), |ui| {
                ui.heading("Rust4Diva Mod Manager");
            });


            egui::ScrollArea::vertical()
                .auto_shrink(false).drag_to_scroll(false)
                .max_height(ctx.input(|i: &egui::InputState| i.screen_rect()).size().y - 150.0)
                .show(ui, |s| {
                    TableBuilder::new(s)
                        .drag_to_scroll(false).vscroll(false)
                        .resizable(true).auto_shrink(true)
                        .columns(Column::initial(30.0).at_least(95.0), 1)
                        .columns(Column::initial(300.0).at_least(300.0), 2)
                        .columns(Column::initial(100.0), 1)
                        .column(Column::remainder())
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                ui.heading("Enabled");
                            });
                            header.col(|ui| {
                                ui.heading("Mod");
                            });
                            header.col(|ui| {
                                ui.heading("Author(s)");
                            });
                            header.col(|ui| {
                                ui.heading("Version");
                            });
                            header.col(|ui| {
                                ui.heading("Description");
                            });
                        })
                        .body(|mut body| {
                            body.rows(40.0, self.mods.len(), |mut row| {
                                let mut divamod = &mut self.mods[row.index()];
                                let mod_path = &divamod.path;
                                let mut config = &mut divamod.config;
                                row.col(|ui| {
                                    ui.with_layout(Layout::top_down_justified(Center), |ui| {
                                        let text = if config.enabled { "Yes" } else { "No" };
                                        if ui.checkbox(&mut config.enabled, text).clicked() {
                                            save_mod_config(mod_path, config);
                                        }
                                    });
                                });
                                row.col(|ui| {
                                    ui.with_layout(Layout::top_down_justified(Min), |ui| {
                                        ui.label(&config.name);
                                    });
                                });
                                row.col(|ui| { ui.label(&config.author); });
                                row.col(|ui| { ui.label(&config.version); });
                                row.col(|ui| { ui.label(&config.description); });
                            });
                        });
                });
            ui.horizontal_centered(|hui| {
                if hui.button("Reload").clicked() {
                    self.mods = load_mods(self);
                    self.loaded = true;
                }
                if hui.button("Save").clicked() {
                    save_mod_configs(self);
                    self.loaded = true;
                }
                let dl_mod = hui.button("Download from GB");
                if dl_mod.clicked() {
                    self.show_dl = true;
                }
            });
        });

        if self.show_dl {
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("dl_win"),
                egui::ViewportBuilder::default()
                    .with_title("Download Mod")
                    .with_inner_size([800.0, 600.0]),
                |ctx, class| {
                    assert!(
                        class == egui::ViewportClass::Immediate,
                        "This egui backend doesn't support multiple viewports"
                    );

                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            let dl_mod = ui.label("Download URL: ");
                            let res = ui.text_edit_singleline(&mut self.dl_mod_url);
                            res.labelled_by(dl_mod.id);
                        });
                        if ui.button("Get info").clicked() {
                            // println!("{}", self.dl_mod_url);
                            let gb_mod = fetch_mod_data(self.dl_mod_url.as_str());
                            if gb_mod.is_some() {
                                self.downloads.push(gb_mod.unwrap());
                            } else {
                                println!("Could not get mod info");
                            }
                        }
                        for mut gb_mod in self.downloads.clone().iter() {
                            ui.horizontal(|h| {
                                h.label(&gb_mod.name);
                                h.label("Files: ");
                            });
                            for mut file in gb_mod.files.iter() {
                                ui.horizontal(|h| {
                                    let dll = h.label(format!("{}", file._sFile));
                                    let mut dl = h.button("Install This file").labelled_by(dll.id);
                                    if dl.clicked() && !self.dl_progress.contains_key(&file._sFile.to_string()) {
                                        let prog_rx = reqwest_mod_data(file, self.dl_done_tx.clone());
                                        self.dl_progress.insert(file._sFile.to_string(), prog_rx);
                                    }
                                    if self.dl_progress.contains_key(&file._sFile) {
                                        let mut rx = self.dl_progress.get_mut(&file._sFile).unwrap();
                                        if let Ok(prog) = rx.try_recv() {
                                            let prog_float = file._nFilesize as f64 / prog as f64;
                                            println!("DL Progress: {}", prog_float);
                                            let progress = ProgressBar::new(prog_float as f32);
                                            h.add(progress);
                                        }
                                    }
                                });
                            }
                            if ui.checkbox(&mut self.should_dl, "show dl test").clicked() {}
                            if self.dlthreads.len() > 0 {
                                if self.should_dl {
                                    let (_handle, show_tx) = &self.dlthreads.last().unwrap();
                                    let _ = show_tx.send(ctx.clone());
                                    for _ in 0..self.dlthreads.len() {
                                        let _ = self.dl_done_rc.recv();
                                    }
                                }
                            }
                        }
                    });

                    if ctx.input(|i| i.viewport().close_requested()) {
                        // Tell parent viewport that we should not show next frame:
                        self.show_dl = false;
                    }
                })
        }
    }
}

