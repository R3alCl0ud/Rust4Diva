# Rust4Diva [![release](https://github.com/R3alCl0ud/Rust4Diva/actions/workflows/release.yml/badge.svg)](https://github.com/R3alCl0ud/Rust4Diva/actions/workflows/release.yml) [![debug](https://github.com/R3alCl0ud/Rust4Diva/actions/workflows/debug.yml/badge.svg)](https://github.com/R3alCl0ud/Rust4Diva/actions/workflows/debug.yml) [![]("https://gamebanana.com/tools/embeddables/17766?type=medium")](https://gamebanana.com/tools/17766)

A Fast cross-platform mod manager written in rust for the steam version of Hatsune Miku Project Diva Mega Mix +



Toggle and set mod priority
![Main Screen of R4D](https://rust4diva.xyz/media/dark/main-v040.png)

Get new mods online directly in the app

![Online Search and DL Screen](https://rust4diva.xyz/media/light/search-grid-v040.png)

# Planned Features / To-Do List
 * [ ] Change how global `All Mods` priority is stored to better support multiple installs of MM+

# Steam Deck Users Scaling issue
 - Add Rust4Diva to steam as a non-steam game
 - Add `'--env=SLINT_SCALE_FACTOR=0.75'` to launch options right after `run`

## Modpack JSON Format
```json
{
  "name": "Example Pack",
  "mods": [
    {
      "name": "test",
      "enabled": true,
      "path":"/absolute/path/to/mod"
    }
  ]
}
```

# Build Instructions

## Linux
install libarchive & git & rust (rustup recommended) from package manager
```sh
git clone https://github.com/R3alCl0ud/Rust4Diva.git
cd Rust4Diva
cargo build
cargo run # runs the built application
```

## Windows
install vcpkg<br>
install libarchive using vcpkg (Note, build will fail if the path to vcpkg has a space in it)
```sh
cargo build
cargo run # runs the built application
```

## Mac OS
install brew<br>
install git<br>
```sh
brew install libarchive
brew install rustup
git clone https://github.com/R3alCl0ud/Rust4Diva.git
cd Rust4Diva
PKG_CONFIG_PATH="/usr/local/opt/libarchive/lib/pkgconfig" cargo build
cargo run # runs the built application
```

# License
Code: GPL-3.0 unless specified

Logo: All Rights Reserved (Use allowed in packaging the application)
Logo Artist: Shibabe

UI Icons: Copyright Font Awesome