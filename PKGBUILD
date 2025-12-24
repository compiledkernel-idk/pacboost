# Maintainer: compiledkernel-idk and pacboost contributors

pkgname=pacboost
pkgver=1.4.3
pkgrel=1
pkgdesc="High-performance Arch Linux package manager frontend leveraging kdownload"
arch=('x86_64')
url="https://github.com/compiledkernel-idk/pacboost"
license=('GPL3')
depends=('gcc-libs' 'glibc' 'pacman')
makedepends=('cargo' 'rust')
source=('git+https://github.com/compiledkernel-idk/pacboost.git') # Placeholder URL
sha256sums=('SKIP')

build() {
  cd "$srcdir/$pkgname"
  
  echo "Building kdownload..."
  cd kdownload
  cargo build --release --locked
  cd ..

  echo "Building pacboost..."
  cargo build --release --locked
}

package() {
  cd "$srcdir/$pkgname"
  
  # Install binaries
  install -Dm755 kdownload/target/release/kdownload "$pkgdir/usr/bin/kdownload"
  install -Dm755 target/release/pacboost "$pkgdir/usr/bin/pacboost"
  
  # Install documentation
  install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
  install -Dm644 assets/logo.svg "$pkgdir/usr/share/pixmaps/pacboost.svg"
}
