function install_prebuilt_circom() {
  mkdir -p ~/bin
  pushd ~/bin
  wget https://github.com/fluidex/static_files/raw/master/circom
  chmod +x circom
  popd
}

function install_circom_from_source() {
  mkdir -p ~/bin
  git clone https://github.com/iden3/circom.git
  pushd circom
  cargo build --release
  cargo install --path circom
  popd
}

install_prebuilt_circom
