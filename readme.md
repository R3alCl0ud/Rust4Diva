# Rust4Diva [![release](https://github.com/R3alCl0ud/Rust4Diva/actions/workflows/release.yml/badge.svg)](https://github.com/R3alCl0ud/Rust4Diva/actions/workflows/main.yml) [![debug](https://github.com/R3alCl0ud/Rust4Diva/actions/workflows/debug.yml/badge.svg)](https://github.com/R3alCl0ud/Rust4Diva/actions/workflows/main.yml)

A Fast cross-platform mod manager written in rust for the steam version of Hatsune Miku Project Diva Mega Mix +

Toggle mods

![Main Screen of R4D](https://rust4diva.xyz/media/dark/main-v040.png)

Get new mods online directly in the app

![Online Search and DL Screen](https://rust4diva.xyz/media/light/search-grid-v040.png)

# Planned Features / To-Do List
 * [ ] Change how global `All Mods` priority is stored to better support multiple installs of MM+
 
## Modpack JSON Format
```json
{
  "name": "Example Pack",
  "mods": [
    {
      "name": "test",
      "enabled": true, // this currently doesn't do anything
      "path":"/absolute/path/to/mod"
    }
  ]
}
```

# Build Instructions

## Linux
install libarchive & git & rust (rustup recommended) from package manager
```
git clone https://github.com/R3alCl0ud/Rust4Diva.git
cd Rust4Diva
cargo build
```

## Windows
install vcpkg<br>
install libarchive using vcpkg (Note, build will fail if the path to vcpkg has a space in it)
```
cargo build
```

## Mac OS
install brew<br>
install git<br>
```
brew install libarchive
brew install rustup
git clone https://github.com/R3alCl0ud/Rust4Diva.git
cd Rust4Diva
PKG_CONFIG_PATH="/usr/local/opt/libarchive/lib/pkgconfig" cargo build
```

# License
Code: GPL-3.0 unless specified

Artwork: All Rights Reserved (Use allowed in packaging the application)
Logo Artist: Shibabe