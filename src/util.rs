use reqwest::{Client, ClientBuilder};

pub fn reqwest_client() -> Client {
    ClientBuilder::new()
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .unwrap()
}
