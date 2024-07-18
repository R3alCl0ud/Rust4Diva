use std::{env, io};
use std::collections::HashMap;
use std::error::Error;
// use egui::DroppedFile;
use interprocess::local_socket::{GenericFilePath, GenericNamespaced, NameType, ToFsName, ToNsName};
use interprocess::local_socket::ListenerOptions;
use interprocess::local_socket::tokio::{prelude::*, Stream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::try_join;

use crate::gamebanana_async::{GBMod, GbModDownload};
use crate::modmanagement::{DivaMod, DivaModLoader};

mod gamebanana_async;
mod modmanagement;

slint::include_modules!();
// slint_build::compile


struct DlProgress {
    rx: Receiver<u64>,
    progress: f32,
}

struct DlFinish {
    success: bool,
    file: GbModDownload,
}
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
    dl_done_rc: Receiver<DlFinish>,
    should_dl: bool,
    dmm_url_rx: Receiver<String>,
    dl_progress: HashMap<String, DlProgress>,
}

#[tokio::main]
async fn main() {
    let (url_tx, mut rx) = tokio::sync::mpsc::channel(2048);
    // rx.try_recv();
    match spawn_listener(url_tx).await {
        Ok(_) => {}
        Err(_) => {}
    }


    let app = App::new().unwrap();
    let app_weak = app.as_weak();
    app.on_clicked(move || {
        let app = app_weak.upgrade().unwrap();
        app.set_counter(app.get_counter() + 1);
    });
    app.run().unwrap();
    println!("Starting Rust4Diva Slint Edition");
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
            dl_done_rc: dl_rx,
            should_dl: false,
            dmm_url_rx: dmm_rx,
            dl_progress: HashMap::new(),
        }
    }
}


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
        let dmm_url = buffer.trim();
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