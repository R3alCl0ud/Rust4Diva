use std::{env, io};
use std::collections::HashMap;
use std::error::Error;
use std::rc::Rc;
use std::sync::Arc;
// use egui::DroppedFile;
use interprocess::local_socket::{GenericFilePath, GenericNamespaced, NameType, ToFsName, ToNsName};
use interprocess::local_socket::ListenerOptions;
use interprocess::local_socket::tokio::{prelude::*, Stream};
use slint::{Model, ModelRc, StandardListViewItem, VecModel, Weak};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use tokio::try_join;

use crate::gamebanana_async::{GBMod, GbModDownload};
use crate::modmanagement::{DivaMod, DivaModLoader, get_diva_folder, load_diva_ml_config, load_mods, load_mods_from_dir};

mod gamebanana_async;
mod modmanagement;

slint::include_modules!();

struct DlProgress {
    rx: Receiver<u64>,
    progress: f32,
}

struct DlFinish {
    success: bool,
    file: GbModDownload,
}
#[derive(Clone)]
struct DivaData {
    mods: Vec<DivaMod>,
    current_dl: Option<GBMod>,
    downloads: Vec<GBMod>,
    mods_directory: String,
    diva_directory: String,
    dl_mod_url: String,
    loaded: bool,
    show_dl: bool,
    dml: DivaModLoader,
    dl_done_tx: Sender<DlFinish>,
    // dl_done_rc: Receiver<DlFinish>,
    should_dl: bool,
    // dmm_url_rx: Receiver<String>,
    // dl_progress: HashMap<String, DlProgress>,
}

#[tokio::main]
async fn main() {
    println!("Starting Rust4Diva Slint Edition");
    let (url_tx, rx) = tokio::sync::mpsc::channel(2048);
    // rx.try_recv();
    match spawn_listener(url_tx).await {
        Ok(_) => {}
        Err(_) => {}
    }

    let mut diva_state = DivaData::new(rx);

    diva_state.diva_directory = get_diva_folder().expect("Unable to get the diva directory");

    diva_state.dml = load_diva_ml_config(&diva_state.diva_directory.as_str()).unwrap();
    diva_state.mods = load_mods(&diva_state);

    diva_state.loaded = true;
    let app = App::new().unwrap();
    let app_weak = app.as_weak();

    push_mods_to_table(&diva_state.mods, app_weak.clone());
    println!("gets past 75");
    let mods_dir = format!("{}/{}", &diva_state.diva_directory.as_str().to_owned(),
                           &diva_state.dml.mods.as_str());
    println!("gets past 77");
    let mut diva_arc = Arc::new(Mutex::new(diva_state));
    app.on_load_mods(move || {
        let app = app_weak.upgrade().unwrap();
        app.set_counter(app.get_counter() + 1);
        let mods = load_mods_from_dir(mods_dir.clone());
        let mut model_vec: VecModel<ModelRc<StandardListViewItem>> = VecModel::default();
        for item in &mods {
            let items: Rc<VecModel<StandardListViewItem>> = Rc::new(VecModel::default());
            let enable_str = if item.config.enabled { "Enabled" } else { "Disabled" };
            let enabled = StandardListViewItem::from(enable_str);
            let name = StandardListViewItem::from(item.config.name.as_str());
            let authors = StandardListViewItem::from(item.config.author.as_str());
            let version = StandardListViewItem::from(item.config.version.as_str());
            let description = StandardListViewItem::from(item.config.description.as_str());
            items.push(enabled);
            items.push(name);
            items.push(authors);
            items.push(version);
            items.push(description);
            model_vec.push(items.into());
        }
        let model = ModelRc::new(model_vec);
        app.set_stuff(model);
        let mut darc = Arc::clone(&diva_arc);
        tokio::spawn(async move {
            darc.lock().await.mods = mods.clone();
        });
    });
    // let app_weak = app.as_weak();
    // let value = Arc::clone(&diva_arc);
    // app.on_toggle_mod(move |row_num| {
    //     let app = app_weak.upgrade().unwrap();
    //     // let mod_table = app.get_mod_table();
    //     println!("Selected row: {}", row_num);
    //
    //     if row_num > -1 {
    //         // let value = Arc::clone(&diva_arc);
    //         let t: usize = row_num as usize;
    //         let value = value.clone();
    //         tokio::spawn(async move {
    //             let binding = Arc::clone(&value);
    //             let mut darc = binding.lock().await;
    //             let module = &darc.mods[t];
    //             println!("Selected Mod = {}", module.config.name);
    //         });
    //     }
    // });
    let app_weak = app.as_weak();
    app.on_add_mod(move || {
        let app = app_weak.upgrade().unwrap();
        let binding = app.get_stuff();
        if let Some(model_vec) = binding.as_any().downcast_ref::<VecModel<slint::ModelRc<StandardListViewItem>>>() {
            let items: Rc<VecModel<StandardListViewItem>> = Rc::new(VecModel::default());
            let enabled = StandardListViewItem::from("enabled");
            let name = StandardListViewItem::from("dummy mod");
            let authors = StandardListViewItem::from("neptune");
            let version = StandardListViewItem::from("420.69");
            let description = StandardListViewItem::from("this is a dummy item because I'm not sure otherwise on how to add items");
            items.push(enabled);
            items.push(name);
            items.push(authors);
            items.push(version);
            items.push(description);
            model_vec.push(items.into());
        }
    });
    println!("Does the app run?");
    app.run().unwrap();
}


fn push_mods_to_table(mods: &Vec<DivaMod>, weak: Weak<App>) {
    let app = weak.upgrade().unwrap();
    let binding = app.get_stuff();
    if let Some(model_vec) = binding.as_any().downcast_ref::<VecModel<slint::ModelRc<StandardListViewItem>>>() {
        for item in mods {
            let items: Rc<VecModel<StandardListViewItem>> = Rc::new(VecModel::default());
            let enable_str = if item.config.enabled { "Enabled" } else { "Disabled" };
            let enabled = StandardListViewItem::from(enable_str);
            let name = StandardListViewItem::from(item.config.name.as_str());
            let authors = StandardListViewItem::from(item.config.author.as_str());
            let version = StandardListViewItem::from(item.config.version.as_str());
            let description = StandardListViewItem::from(item.config.description.as_str());
            items.push(enabled);
            items.push(name);
            items.push(authors);
            items.push(version);
            items.push(description);
            model_vec.push(items.into());
        }
    }
}

impl DivaData {
    fn new(dmm_rx: Receiver<String>) -> Self {
        let (dl_tx, dl_rx) = tokio::sync::mpsc::channel::<DlFinish>(2048);
        Self {
            mods: Vec::new(),
            downloads: Vec::new(),
            current_dl: None,
            loaded: false,
            mods_directory: "".to_string(),
            dl_mod_url: "524621".to_string(),
            show_dl: false,
            diva_directory: "".to_string(),
            dml: DivaModLoader {
                enabled: false,
                console: false,
                mods: "".to_string(),
                version: "".to_string(),
            },
            dl_done_tx: dl_tx,
            // dl_done_rc: dl_rx,
            should_dl: false,
            // dmm_url_rx: dmm_rx,
            // dl_progress: HashMap::new(),
        }
    }
    fn load_mods(&mut self) -> &Vec<DivaMod> {
        self.mods = load_mods(self);
        return &self.mods;
    }
}


/// This is the function for the url handling, should this return Result(True) we know that we are
/// the listening server and should run the display window
async fn spawn_listener(dmm_url_tx: Sender<String>) -> Result<bool, Box<dyn Error>> {
    println!("Starting dmm url listener");


    let print_name = "rust4diva.sock";
    // Pick a name.
    let name = if GenericNamespaced::is_supported() {
        print_name.to_ns_name::<GenericNamespaced>()?
    } else {
        format!("/tmp/{}", print_name).to_fs_name::<GenericFilePath>()?
    };

    // Await this here since we can't do a lot without a connection.
    // let conn = Stream::connect(name).await;

    // let send_url = dmm_url_tx.clone();
    async fn handle_conn(conn: Stream, send_url: Sender<String>) -> io::Result<()> {
        let mut reciever = BufReader::new(&conn);
        let mut sender = &conn;

        // Allocate a sizeable buffer for receiving. This size should be big enough and easy to
        // find for the allocator.
        let mut buffer = String::with_capacity(128);

        // Describe the send operation as sending our whole message.
        let send = sender.write_all(b"URL Recieved\n");
        // Describe the receive operation as receiving a line into our big buffer.
        let recv = reciever.read_line(&mut buffer);

        // Run both operations concurrently.
        try_join!(recv, send)?;

        // Produce our output!
        println!("DMM Url: {}", buffer.trim());
        // let dmm_str = buffer.trim().clone().to_owned();
        let dmm_url = buffer.trim();
        // dmm_url_tx
        send_url.send(dmm_url.to_string()).await.expect("unable to transmit the received url to main thread");

        Ok(())
    }


    // let name = print_name.to_ns_name::<GenericNamespaced>()?;

    // Configure our listener...
    let opts = ListenerOptions::new().name(name);

    // ...and create it.
    let listener = match opts.create_tokio() {
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            eprintln!(
                "
Error: could not start server because the socket file is occupied. Please check if {print_name}
is in use by another process and try again."
            );
            return Err(e.into());
        }
        x => x?,
    };

    // The synchronization between the server and client, if any is used, goes here.
    eprintln!("Server running at {print_name}");
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