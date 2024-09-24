if [[ -e ./.flatpak-builder ]]; then
  rm -rf ./.flatpak-builder
fi
if [[ ! -e flatpak-cargo-generator.py ]]; then
  wget https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
fi
if [[ ! -e pyproject.toml ]]; then
  wget https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/pyproject.toml
else 
  echo "pyproject already installed"
fi

source $(poetry env info --path)/bin/activate
poetry install 
python3 ./flatpak-cargo-generator.py ../Cargo.lock -o rust4diva-sources.json 
rm -rf ./diva_repo
mkdir ./diva_repo
flatpak run org.flatpak.Builder --force-clean --sandbox --user --install --ccache --mirror-screenshots-url=https://dl.flathub.org/media/ --repo=diva_repo repo xyz.rust4diva.Rust4Diva.json
# flatpak-builder --repo=diva_repo repo xyz.rust4diva.Rust4Diva.json --force-clean --user -y --mirror-screenshots-url=https://rust4diva.xyz/media/
flatpak build-bundle ./diva_repo rust4diva.flatpak xyz.rust4diva.Rust4Diva
# flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest xyz.rust4diva.Rust4Diva.json
# flatpak run --command=flatpak-builder-lint org.flatpak.Builder repo diva_repo
