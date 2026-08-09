# Maintainer: Mike Ravenblack <0xRavenBlack@github>
pkgname=shadowdate
_appid=0xravenblack.shadowdata
pkgver=0.6.5
pkgrel=1
pkgdesc="A gothic dark-pastel desktop calendar for Linux (Rust + GTK4) with iCalendar support and a background reminder service"
arch=('x86_64')
url="https://github.com/0xRavenBlack/ShadowDate"
options=('!debug')
license=('MIT')
depends=('gtk4' 'glib2')
# The AUR package is just this PKGBUILD: every non-binary source (desktop
# entry, icon, systemd unit, license) is harvested from the repository at the
# release tag via raw.githubusercontent.com, so no local files are needed.
source=("${pkgname}::https://github.com/0xRavenBlack/ShadowDate/releases/download/v${pkgver}/shadowdate-${pkgver}-x86_64-linux"
        "${pkgname}-service::https://github.com/0xRavenBlack/ShadowDate/releases/download/v${pkgver}/shadowdate-service-${pkgver}-x86_64-linux"
        "0xravenblack.shadowdata.desktop::https://raw.githubusercontent.com/0xRavenBlack/ShadowDate/v${pkgver}/0xravenblack.shadowdata.desktop"
        "logo.svg::https://raw.githubusercontent.com/0xRavenBlack/ShadowDate/v${pkgver}/resources/svg/logo.svg"
        "shadowdate-service.service::https://raw.githubusercontent.com/0xRavenBlack/ShadowDate/v${pkgver}/shadowdate-service.service"
        "LICENSE::https://raw.githubusercontent.com/0xRavenBlack/ShadowDate/v${pkgver}/LICENSE")
sha256sums=('713b90f04bf356498e19d1f5f4065eb6947dac51142f81606c277faf1af15c97'
            '793d50d2f415159868f0864a3df347c7da6ee7de2e7b5eea2d368a8e253da3b5'
            '8a9962947917a207b2648c9be705f5e6c2f64120085bf514929fe11c20ca8b6e'
            '3c0e4fbf0037795dcc9013a871cf18a289ab183ac0809111e74a58580d0b1c03'
            '1b0a55a83d591047c5e244a40f991fbb151bf98aeb39a5856272731442043301'
            '32c7d774d68ff9f1adc053fd55c2ac1a1c0f31bbfabed2528acb96c05ce64e00')

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
