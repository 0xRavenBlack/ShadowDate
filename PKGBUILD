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
# Local sources ship flat next to the PKGBUILD (makepkg resolves local sources
# by basename in the build directory), mirroring 0xravenblack.shadowdata.desktop
# and LICENSE at the repo root.
source=("${pkgname}::https://github.com/0xRavenBlack/ShadowDate/releases/download/v${pkgver}/shadowdate-${pkgver}-x86_64-linux"
        "${pkgname}-service::https://github.com/0xRavenBlack/ShadowDate/releases/download/v${pkgver}/shadowdate-service-${pkgver}-x86_64-linux"
        "0xravenblack.shadowdata.desktop"
        "logo.svg"
        "shadowdate-service.service"
        "LICENSE")
sha256sums=('20a3758963b1cbd456c361907c95bd141e9f2b9f4ed6b46cb6a42242f9180f85'
            '34718eab928ebdf561236df9346661413d8eb1064d3c387f1116c2e032cf2bfc'
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
    install -Dm644 "logo.svg" \
        "${pkgdir}/usr/share/icons/hicolor/scalable/apps/${_appid}.svg"

    # Systemd user unit for the background reminder service
    install -Dm644 "shadowdate-service.service" \
        "${pkgdir}/usr/lib/systemd/user/shadowdate-service.service"

    # License
    install -Dm644 "LICENSE" "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"
}
