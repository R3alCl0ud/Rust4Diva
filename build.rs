use std::{env, io};

use winresource::WindowsResource;

fn main() -> io::Result<()> {
    let config = slint_build::CompilerConfiguration::new().with_style("cosmic".into());
    slint_build::compile_with_config("ui/appwindow.slint", config).unwrap();

    if env::var_os("CARGO_CFG_WINDOWS").is_some() {
        WindowsResource::new()
            // This path can be absolute, or relative to your crate root.
            .set_icon("assets/rust4diva.ico")
            .compile()?;
    }
    Ok(())
}
