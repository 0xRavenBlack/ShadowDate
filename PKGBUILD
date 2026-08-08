# Maintainer: Mike Ravenblack <0xRavenBlack@github>
pkgname=shadowdate
_appid=0xravenblack.shadowdata
pkgver=0.5.0
pkgrel=1
pkgdesc="A gothic dark-pastel desktop calendar for Linux (Rust + GTK4) with iCalendar support and a background reminder service"
arch=('x86_64')
url="https://github.com/0xRavenBlack/ShadowDate"
options=('!debug')
license=('MIT')
depends=('gtk4' 'glib2')
# TODO: regenerate sha256sums with `makepkg -g` after uploading the v0.5.0
# release assets (shadowdate + shadowdate-service binaries). The placeholder
# sums below are the sha256 of the empty string.
source=("${pkgname}::https://github.com/0xRavenBlack/ShadowDate/releases/download/v${pkgver}/shadowdate-${pkgver}-x86_64-linux"
        "${pkgname}-service::https://github.com/0xRavenBlack/ShadowDate/releases/download/v${pkgver}/shadowdate-service-${pkgver}-x86_64-linux"
        "0xravenblack.shadowdata.desktop"
        "resources/svg/logo.svg"
        "resources/shadowdate-service.service"
        "LICENSE")
sha256sums=('e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
            'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
            'SKIP'
            'SKIP'
            'SKIP'
            'SKIP')

package() {
    cd "${srcdir}"

    # Prebuilt release binaries (shadowdate-<pkgver>-x86_64-linux)
    install -Dm755 "shadowdate" "${pkgdir}/usr/bin/${pkgname}"
    install -Dm755 "shadowdate-service" "${pkgdir}/usr/bin/${pkgname}-service"

    # Desktop entry
    install -Dm644 "0xravenblack.shadowdata.desktop" \
        "${pkgdir}/usr/share/applications/${_appid}.desktop"

    # Icon (logo.svg is a vector SVG; installed as the scalable themed icon)
    install -Dm644 "resources/svg/logo.svg" \
        "${pkgdir}/usr/share/icons/hicolor/scalable/apps/${_appid}.svg"

    # Systemd user unit for the background reminder service
    install -Dm644 "resources/shadowdate-service.service" \
        "${pkgdir}/usr/lib/systemd/user/shadowdate-service.service"

    # License
    install -Dm644 "LICENSE" "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"
}
