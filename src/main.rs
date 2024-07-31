use std::{env, io};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

use futures_util::SinkExt;
use interprocess::local_socket::{GenericFilePath, GenericNamespaced, NameType, ToFsName, ToNsName};
use interprocess::local_socket::ListenerOptions;
use interprocess::local_socket::tokio::{prelude::*, Stream};
use slint::Weak;
use slint_interpreter::ComponentHandle;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::mpsc::error::SendError;
use tokio::sync::Mutex;
use tokio::try_join;

use crate::gamebanana_async::{GbModDownload, GBSearch};
use crate::modmanagement::{create_tmp_if_not, DivaMod, DivaModLoader, get_diva_folder, load_diva_ml_config, load_mods, push_mods_to_table};

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
    search_results: Vec<GBSearch>,
    mods_directory: String,
    diva_directory: String,
    dl_mod_url: String,
    dml: Option<DivaModLoader>,
    dl_done_tx: Sender<DlFinish>,
    dl_queue: Arc<Mutex<Vec<Download>>>,
    mod_files: HashMap<u64, Vec<GbModDownload>>,
}

#[tokio::main]
async fn main() {
    println!("Starting Rust4Diva Slint Edition");
    create_tmp_if_not().expect("Failed to create temp directory, now we are panicking");

    let app = App::new().unwrap();
    let app_weak = app.as_weak();


    let (url_tx, url_rx) = tokio::sync::mpsc::channel(2048);
    let (dl_tx, dl_rx) = tokio::sync::mpsc::channel::<(i32, Download)>(2048);


    // rx.try_recv();
    match spawn_listener(url_tx, app_weak.clone()).await {
        Ok(_) => {}
        Err(_) => {}
    }


    let mut diva_state = DivaData::new();

    diva_state.diva_directory = get_diva_folder().expect("Unable to get the diva directory");

    diva_state.dml = load_diva_ml_config(&diva_state.diva_directory.as_str());
    diva_state.mods = load_mods(&diva_state);


    push_mods_to_table(&diva_state.mods, app_weak.clone());

    if let Some(dml) = &diva_state.dml {
        app.set_dml_version(dml.version.clone().into());
        app.set_dml_enabled(dml.enabled);
    }


    let diva_arc = Arc::new(Mutex::new(diva_state));
    modmanagement::init(&app, Arc::clone(&diva_arc), dl_rx).await;
    gamebanana_async::init(&app, Arc::clone(&diva_arc), dl_tx, url_rx).await;
    println!("Does the app run?");
    // app.
    app.run().unwrap();
}


impl DivaData {
    fn new() -> Self {
        let (dl_tx, dl_rx) = tokio::sync::mpsc::channel::<DlFinish>(2048);
        Self {
            mods: Vec::new(),
            search_results: Vec::new(),
            mods_directory: "".to_string(),
            dl_mod_url: "524621".to_string(),
            diva_directory: "".to_string(),
            dml: None,
            dl_done_tx: dl_tx,
            dl_queue: Arc::new(Mutex::new(Vec::new())),
            mod_files: HashMap::new(),
        }
    }
}


/// This is the function for the url handling, should this return Result(True) we know that we are
/// the listening server and should run the display window
async fn spawn_listener(dmm_url_tx: Sender<String>, weak: Weak<App>) -> Result<bool, Box<dyn Error>> {
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
        match send_url.send(dmm_url.to_string()).await {
            Ok(_) => {}
            Err(e) => {
                eprintln!("main @ 157: {}", e);
            }
        }

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
        }
    });
    Ok(true)
}