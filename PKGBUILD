# Maintainer: Mike Ravenblack <0xRavenBlack@github>
pkgname=shadowdate
_appid=0xravenblack.shadowdata
pkgver=0.4.1
pkgrel=1
pkgdesc="A gothic dark-pastel desktop calendar for Linux (Rust + GTK4) with iCalendar support"
arch=('x86_64')
url="https://github.com/0xRavenBlack/ShadowDate"
options=('!debug')
license=('MIT')
depends=('gtk4' 'glib2')
source=("${pkgname}::https://github.com/0xRavenBlack/ShadowDate/releases/download/v${pkgver}/shadowdate-${pkgver}-x86_64-linux"
        "0xravenblack.shadowdata.desktop"
        "resources/svg/logo.svg"
        "LICENSE")
sha256sums=('03168388fac7304b84678c928ef2622da564c5a5c85d8b80228b89f4f2011df2'
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

    # Icon (logo.svg is a vector SVG; installed as the scalable themed icon)
    install -Dm644 "resources/svg/logo.svg" \
        "${pkgdir}/usr/share/icons/hicolor/scalable/apps/${_appid}.svg"

    # License
    install -Dm644 "LICENSE" "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"
}
