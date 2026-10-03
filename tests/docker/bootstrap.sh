#!/bin/sh
# Run inside a disposable Debian Rust container; source is mounted read-only at /source.
set -eu
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq tmux fish zsh bash-completion python3 procps curl ca-certificates make gcc libc6-dev libncurses-dev > /tmp/packages.log
mkdir -p /work
cp -R /source/src /source/shell /source/tests /source/Cargo.toml /source/Cargo.lock /source/rfig.zsh /source/rfig.bash /source/rfig.fish /source/install.sh /work/
cd /work
cargo build --locked -j 2
cargo test --locked -j 2
if [ "${BUILD_BASH44:-0}" = 1 ]; then
  curl --fail --location --retry 2 --max-time 90 https://ftp.gnu.org/gnu/bash/bash-4.4.tar.gz -o /tmp/bash-4.4.tar.gz
  sha256sum /tmp/bash-4.4.tar.gz > /work/bash44-source.sha256
  tar -xzf /tmp/bash-4.4.tar.gz -C /tmp
  cd /tmp/bash-4.4
  ./configure --prefix=/opt/bash44 --without-bash-malloc > /tmp/bash44-configure.log
  make -j 2 > /tmp/bash44-build.log 2>&1
  make install > /tmp/bash44-install.log 2>&1
fi
useradd -m -s /bin/bash tester
chown -R tester:tester /work
