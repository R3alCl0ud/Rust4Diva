#./gensources-flatpak.sh
rm -rf ./diva_repo
mkdir ./diva_repo
flatpak run org.flatpak.Builder --force-clean --user --install --ccache --mirror-screenshots-url=https://dl.flathub.org/media/ --repo=diva_repo repo xyz.rust4diva.Rust4Diva.json
# flatpak-builder --repo=diva_repo repo xyz.rust4diva.Rust4Diva.json --force-clean --user -y --mirror-screenshots-url=https://rust4diva.xyz/media/
flatpak build-bundle ./diva_repo rust4diva.flatpak xyz.rust4diva.Rust4Diva
# flatpak run --command=flatpak-builder-lint org.flatpak.Builder manifest xyz.rust4diva.Rust4Diva.json
# flatpak run --command=flatpak-builder-lint org.flatpak.Builder repo diva_repo
