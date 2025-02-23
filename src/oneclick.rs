use std::error::Error;
use std::io;

use crate::{
    diva::open_error_window,
    divamodarchive::{self, parse_oneclick_url},
    downloads, gamebanana, App, Download, GameBananaLogic, SearchPreviewData, SearchProvider,
};
use interprocess::local_socket::{
    tokio::{prelude::*, Stream},
    GenericFilePath, GenericNamespaced,
};
use interprocess::local_socket::{ListenerOptions, NameType, ToFsName, ToNsName};
use slint::{private_unstable_api::re_exports::ColorScheme, Model};
use slint::{ComponentHandle, ModelRc, ToSharedString, VecModel, Weak};
use tokio::sync::mpsc::Sender;
use tokio::sync::{broadcast, mpsc::Receiver};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    try_join,
};

/// This is the function for the url handling, should this return Result(True) we know that we are
/// the listening server and should run the display window
pub async fn spawn_listener(
    dmm_url_tx: Sender<String>,
    _weak: Weak<App>,
) -> Result<bool, Box<dyn Error>> {
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

pub async fn try_send_mmdl(dmm: String) -> Result<(), Box<dyn Error>> {
    let printname = "rust4diva.sock";
    // Pick a name.
    let name = if GenericNamespaced::is_supported() {
        printname.to_ns_name::<GenericNamespaced>()?
    } else {
        "/tmp/rust4diva.sock".to_fs_name::<GenericFilePath>()?
    };

    // Await this here since we can't do a whole lot without a connection.
    let conn = Stream::connect(name).await;

    match conn {
        // we are the url handler
        Ok(conn) => {
            // This consumes our connection and splits it into two halves, so that we can concurrently use
            // both.
            let (receiver, mut sender) = conn.split();
            let mut receiver = BufReader::new(receiver);

            // Allocate a sizeable buffer for receiving. This size should be enough and should be easy to
            // find for the allocator.
            let mut buffer = String::with_capacity(128);

            // Describe the send operation as writing our whole string.
            let send = sender.write_all(dmm.as_str().as_ref());
            // Describe the receive operation as receiving until a newline into our buffer.
            let recv = receiver.read_line(&mut buffer);

            // Concurrently perform both operations.
            try_join!(send, recv)?;

            // Close the connection a bit earlier than you'd think we would. Nice practice!
            drop((receiver, sender));

            // Display the results when we're done!
            println!("Server answered: {}", buffer.trim());
        }

        // are we the server? Just going to assume we are.
        Err(e) => {
            return Err(e.into());
        }
    }
    Ok(())
}

pub fn check_valid_oneclick_url(url: String) -> Option<SearchProvider> {
    if !url.starts_with("divamodmanager:") {
        return None;
    }
    if let Some(split) = url.split_once(":") {
        if split.1.starts_with("dma") {
            return Some(SearchProvider::DivaModArchive);
        }
        if split.1.starts_with("https://gamebanana.com/mmdl/") {
            return Some(SearchProvider::GameBanana);
        }
    }
    None
}

pub fn handle_dmm_oneclick(
    mut url_rx: Receiver<String>,
    ui_handle: Weak<App>,
    dark_rx: broadcast::Receiver<ColorScheme>,
) -> tokio::task::JoinHandle<()> {
    return tokio::spawn(async move {
        while !url_rx.is_closed() {
            if let Some(url) = url_rx.recv().await {
                match check_valid_oneclick_url(url.clone()) {
                    Some(provider) => {
                        let mut to_download = None;
                        let mut gb_item = None;
                        let mut dma_item = None;
                        match provider {
                            SearchProvider::DivaModArchive => {
                                match divamodarchive::parse_oneclick_url(url) {
                                    Some(postid) => {
                                        match divamodarchive::fetch_post(postid).await {
                                            Ok(post) => {
                                                if post.files.len() == 1 {
                                                    to_download = Some(
                                                        (post.id as i64 | ((0 as i64) << 32))
                                                            .to_string(),
                                                    );
                                                }
                                                dma_item = Some(post)
                                            }
                                            Err(e) => {
                                                open_error_window(e.to_string());
                                                continue;
                                            }
                                        }
                                    }
                                    None => continue,
                                }
                            }
                            SearchProvider::GameBanana => {
                                let gbitem = match gamebanana::parse_dmm_url(url) {
                                    Some(item) => item,
                                    None => continue,
                                };
                                to_download = Some(gbitem.file_id.to_string());
                                match gamebanana::fetch_mod(gbitem.item_id).await {
                                    Ok(m) => gb_item = Some(m),
                                    Err(e) => {
                                        open_error_window(e.to_string());
                                        continue;
                                    }
                                }
                            }
                        };

                        let weak = ui_handle.clone();
                        let rx = dark_rx.resubscribe();

                        // if let Ok(item_str) = sonic_rs::to_string(item) {
                        let _ = slint::invoke_from_event_loop(move || {
                            // let item: SearchPreviewData = sonic_rs::from_str(&item_str);
                            let item: SearchPreviewData = match gb_item {
                                Some(gb) => gb.into(),
                                None => match dma_item {
                                    Some(dma) => dma.into(),
                                    None => return,
                                },
                            };

                            let deets = downloads::create_deets_window(item.clone(), weak, rx);
                            deets.set_files(item.files.clone());
                            let files = deets.get_files();
                            if let Some(to_dl) = to_download {
                                let dl_id = to_dl.to_shared_string();
                                for mut file in files.iter() {
                                    if file.id == dl_id {
                                        file.inprogress = true;
                                        deets.global::<GameBananaLogic>().invoke_download(file.clone());
                                        break;
                                    }
                                }
                            }
                            deets.show().unwrap();
                        });
                    }
                    None => continue,
                }
            }
        }
        println!("Oneclick receiver closed");
    });
}
