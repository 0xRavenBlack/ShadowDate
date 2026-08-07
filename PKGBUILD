# Maintainer: Mike Ravenblack <0xRavenBlack@github>
pkgname=shadowdate
_appid=0xravenblack.shadowdata
pkgver=0.4.0
pkgrel=1
pkgdesc="A gothic dark-pastel desktop calendar for Linux (Rust + GTK4) with iCalendar support"
arch=('x86_64')
url="https://github.com/0xRavenBlack/ShadowDate"
options=('!debug')
license=('MIT')
depends=('gtk4' 'glib2')
source=("${pkgname}::https://github.com/0xRavenBlack/ShadowDate/releases/download/v${pkgver}/shadowdate-${pkgver}-x86_64-linux"
        "0xravenblack.shadowdata.desktop"
        "Logo.png"
        "LICENSE")
sha256sums=('92f59b904c18a2701f870678732a797d1ab9d7a17fc82e325b634e4c8c54f32a'
            'SKIP'
            'SKIP'
            'SKIP')

package() {
    cd "${srcdir}"

    # Prebuilt release binary (shadowdate-<pkgver>-x86_64-linux)
    install -Dm755 "shadowdate" "${pkgdir}/usr/bin/${pkgname}"

    # Desktop entry
    install -Dm644 "0xravenblack.shadowdata.desktop" \
        "${pkgdir}/usr/share/applications/${_appid}.desktop"

    # Icon (Logo.png is a 128x128 raster PNG)
    install -Dm644 "Logo.png" \
        "${pkgdir}/usr/share/icons/hicolor/128x128/apps/${_appid}.png"

    # License
    install -Dm644 "LICENSE" "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"
}
