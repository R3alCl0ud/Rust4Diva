use std::{env, io};

#[cfg(debug_assertions)]
use std::process::Command;

use winresource::WindowsResource;

fn main() -> io::Result<()> {
    let config = slint_build::CompilerConfiguration::new()
        .with_style("cosmic".into());
    slint_build::compile_with_config("ui/appwindow.slint", config).unwrap();

    #[cfg(debug_assertions)]
    if let Ok(output) = Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
    {
        let git_hash = match String::from_utf8(output.stdout) {
            Ok(hash) => hash,
            Err(_) => "".to_owned(),
        };

        println!("cargo:rustc-env=GIT_HASH=-git-{}", git_hash);
    } else {
        println!("cargo:rustc-env=GIT_HASH=-git-UNKNOWN");
    }

    #[cfg(not(debug_assertions))]
    println!("cargo:rustc-env=GIT_HASH={}", "");

    if env::var_os("CARGO_CFG_WINDOWS").is_some() {
        WindowsResource::new()
            // This path can be absolute, or relative to your crate root.
            .set_icon("assets/rust4diva.ico")
            .compile()?;
    }
    Ok(())
}
