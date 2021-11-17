function install_prebuilt_circom() {
  mkdir -p ~/bin
  pushd ~/bin
  wget https://github.com/fluidex/static_files/raw/master/circom
  chmod +x circom
  popd
}

function install_circom_from_source() {
  cargo install --git https://github.com/iden3/circom
}

install_prebuilt_circom
