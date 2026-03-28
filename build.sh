#!/usr/bin/env bash
#
# build.sh - Compile MyCommercial (GUI native) et generer les packages .deb et .rpm
#
# Usage:
#   ./build.sh              # Compile + .deb + .rpm
#   ./build.sh build        # Compile uniquement (release)
#   ./build.sh deb          # Compile + .deb uniquement
#   ./build.sh rpm          # Compile + .rpm uniquement
#   ./build.sh all          # Compile + .deb + .rpm
#   ./build.sh install-deps # Installer les outils et dependances systeme
#   ./build.sh clean        # Nettoyer les artefacts
#   ./build.sh check-deps   # Verifier les dependances systeme
#
set -euo pipefail

# ── Configuration ──
PROJECT_NAME="mycommercial"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
ARCH=$(uname -m)
BUILD_DIR="target/release"
DIST_DIR="dist"

# Couleurs
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${CYAN}[INFO]${NC} $*"; }
ok()    { echo -e "${GREEN}[OK]${NC} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }

# ── Dependances systeme egui/eframe (OpenGL + X11/Wayland) ──

# Compile-time deps (needed for cargo build)
DEB_BUILD_DEPS=(
    build-essential
    pkg-config
    dpkg-dev
    liblzma-dev
    # egui/eframe (glutin + winit) needs:
    libxcb-render0-dev
    libxcb-shape0-dev
    libxcb-xfixes0-dev
    libxkbcommon-dev
    libfontconfig1-dev
    libfreetype-dev
    # OpenGL
    libgl-dev
    libegl-dev
    # Wayland (optionnel mais recommande)
    libwayland-dev
    libxcb1-dev
)

# Runtime deps for .deb packages
DEB_RUNTIME_DEPS="libc6 (>= 2.31), libgl1, libegl1, libfontconfig1, libxcb-render0, libxcb-shape0, libxcb-xfixes0, libxkbcommon0"

RPM_BUILD_DEPS=(
    gcc
    make
    rpm-build
    pkgconfig
    libxcb-devel
    libxkbcommon-devel
    fontconfig-devel
    freetype-devel
    mesa-libGL-devel
    mesa-libEGL-devel
    wayland-devel
)

RPM_RUNTIME_DEPS="glibc >= 2.17, mesa-libGL, mesa-libEGL, fontconfig, libxcb, libxkbcommon"

# ── Verifier les prerequis ──
check_rust() {
    if ! command -v cargo &>/dev/null; then
        error "Rust/Cargo non installe. Installez via:"
        error "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
    info "Rust $(rustc --version | cut -d' ' -f2) detecte"
}

check_system_deps() {
    local missing=()

    if command -v dpkg &>/dev/null; then
        info "Verification des dependances Debian/Ubuntu..."
        for pkg in "${DEB_BUILD_DEPS[@]}"; do
            if ! dpkg -s "$pkg" &>/dev/null 2>&1; then
                missing+=("$pkg")
            fi
        done
    elif command -v rpm &>/dev/null; then
        info "Verification des dependances RHEL/Fedora..."
        for pkg in "${RPM_BUILD_DEPS[@]}"; do
            if ! rpm -q "$pkg" &>/dev/null 2>&1; then
                missing+=("$pkg")
            fi
        done
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        warn "Dependances manquantes: ${missing[*]}"
        warn "Lancez './build.sh install-deps' pour les installer"
        return 1
    else
        ok "Toutes les dependances systeme sont presentes"
        return 0
    fi
}

check_cargo_deb() {
    if ! command -v cargo-deb &>/dev/null; then
        warn "cargo-deb non installe"
        return 1
    fi
    return 0
}

check_cargo_rpm() {
    if ! command -v cargo-generate-rpm &>/dev/null; then
        warn "cargo-generate-rpm non installe"
        return 1
    fi
    return 0
}

# ── Installer les dependances ──
install_deps() {
    info "Installation des dependances systeme et outils de packaging..."
    echo ""

    if command -v apt-get &>/dev/null; then
        info "Systeme Debian/Ubuntu detecte"
        sudo apt-get update -qq
        info "Installation des dependances de compilation (egui/OpenGL/X11/Wayland)..."
        sudo apt-get install -y -qq "${DEB_BUILD_DEPS[@]}" 2>/dev/null || true
        ok "Dependances systeme installees"
    elif command -v dnf &>/dev/null; then
        info "Systeme Fedora/RHEL detecte"
        sudo dnf install -y "${RPM_BUILD_DEPS[@]}" 2>/dev/null || true
        ok "Dependances systeme installees"
    elif command -v yum &>/dev/null; then
        info "Systeme CentOS/RHEL detecte"
        sudo yum install -y "${RPM_BUILD_DEPS[@]}" 2>/dev/null || true
        ok "Dependances systeme installees"
    else
        warn "Gestionnaire de paquets non reconnu. Installez manuellement:"
        warn "  OpenGL dev, X11/XCB dev, fontconfig dev, libxkbcommon dev"
    fi

    echo ""
    info "Installation de cargo-deb..."
    cargo install cargo-deb 2>/dev/null || warn "Echec installation cargo-deb"

    info "Installation de cargo-generate-rpm..."
    cargo install cargo-generate-rpm 2>/dev/null || warn "Echec installation cargo-generate-rpm"

    echo ""
    ok "Installation terminee"
}

# ── Compilation ──
build_release() {
    info "Compilation en mode release (GUI native egui/eframe)..."
    cargo build --release 2>&1

    if [ ! -f "${BUILD_DIR}/${PROJECT_NAME}" ]; then
        error "Le binaire n'a pas ete genere"
        exit 1
    fi

    local size
    size=$(du -h "${BUILD_DIR}/${PROJECT_NAME}" | cut -f1)
    ok "Binaire compile: ${BUILD_DIR}/${PROJECT_NAME} (${size})"

    if command -v strip &>/dev/null; then
        info "Strip des symboles de debug..."
        strip -s "${BUILD_DIR}/${PROJECT_NAME}"
        size=$(du -h "${BUILD_DIR}/${PROJECT_NAME}" | cut -f1)
        ok "Binaire strippe: ${size}"
    fi
}

# ── Generation .deb ──
build_deb() {
    if ! check_cargo_deb; then
        warn "Installez cargo-deb: cargo install cargo-deb"
        warn "Ou lancez: ./build.sh install-deps"
        return 1
    fi

    # Mettre a jour les depends runtime dans Cargo.toml temporairement
    # (cargo-deb lit [package.metadata.deb].depends)
    info "Generation du package .deb..."
    cargo deb --no-build 2>&1

    local deb_file
    deb_file=$(find target/debian -name "*.deb" -type f -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2)

    if [ -n "${deb_file}" ]; then
        mkdir -p "${DIST_DIR}"
        cp "${deb_file}" "${DIST_DIR}/"
        local size
        size=$(du -h "${deb_file}" | cut -f1)
        ok "Package .deb genere: ${deb_file} (${size})"

        if command -v dpkg-deb &>/dev/null; then
            echo ""
            info "Contenu du package:"
            dpkg-deb -I "${deb_file}" 2>/dev/null | head -20 || true
            echo ""
            info "Installation: sudo dpkg -i ${DIST_DIR}/$(basename "${deb_file}")"
        fi
    else
        error "Aucun .deb genere"
        return 1
    fi
}

# ── Generation .rpm ──
build_rpm() {
    if ! check_cargo_rpm; then
        warn "Installez cargo-generate-rpm: cargo install cargo-generate-rpm"
        warn "Ou lancez: ./build.sh install-deps"
        return 1
    fi

    info "Generation du package .rpm..."
    cargo generate-rpm 2>&1

    local rpm_file
    rpm_file=$(find target/generate-rpm -name "*.rpm" -type f -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2)

    if [ -n "${rpm_file}" ]; then
        mkdir -p "${DIST_DIR}"
        cp "${rpm_file}" "${DIST_DIR}/"
        local size
        size=$(du -h "${rpm_file}" | cut -f1)
        ok "Package .rpm genere: ${rpm_file} (${size})"

        echo ""
        info "Installation: sudo rpm -i ${DIST_DIR}/$(basename "${rpm_file}")"
        info "  ou: sudo dnf install ${DIST_DIR}/$(basename "${rpm_file}")"
    else
        error "Aucun .rpm genere"
        return 1
    fi
}

# ── Nettoyage ──
clean() {
    info "Nettoyage..."
    cargo clean 2>/dev/null || true
    rm -rf "${DIST_DIR}"
    ok "Nettoyage termine"
}

# ── Resume ──
summary() {
    echo ""
    echo -e "${GREEN}═══════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}  MyCommercial v${VERSION} (GUI native) - Build termine${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════════════${NC}"
    echo ""

    if [ -d "${DIST_DIR}" ]; then
        info "Packages disponibles dans ${DIST_DIR}/:"
        ls -lh "${DIST_DIR}"/ 2>/dev/null || true
    fi

    echo ""
    info "Le binaire standalone est dans: ${BUILD_DIR}/${PROJECT_NAME}"
    echo ""
    info "Dependances runtime:"
    info "  Debian/Ubuntu: ${DEB_RUNTIME_DEPS}"
    info "  RHEL/Fedora:   ${RPM_RUNTIME_DEPS}"
    echo ""
}

# ── Main ──
main() {
    local cmd="${1:-all}"

    echo -e "${CYAN}╔═══════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║  MyCommercial v${VERSION} - Build System (GUI native)   ║${NC}"
    echo -e "${CYAN}╚═══════════════════════════════════════════════════╝${NC}"
    echo ""

    check_rust

    case "${cmd}" in
        build)
            build_release
            ;;
        deb)
            build_release
            build_deb
            summary
            ;;
        rpm)
            build_release
            build_rpm
            summary
            ;;
        all|"")
            build_release
            echo ""
            build_deb || warn "Skipping .deb (cargo-deb non disponible)"
            echo ""
            build_rpm || warn "Skipping .rpm (cargo-generate-rpm non disponible)"
            summary
            ;;
        install-deps)
            install_deps
            ;;
        check-deps)
            check_system_deps
            ;;
        clean)
            clean
            ;;
        help|-h|--help)
            echo "Usage: $0 [commande]"
            echo ""
            echo "Commandes:"
            echo "  build        Compiler le projet en mode release"
            echo "  deb          Compiler + generer le package .deb"
            echo "  rpm          Compiler + generer le package .rpm"
            echo "  all          Compiler + generer .deb et .rpm (defaut)"
            echo "  install-deps Installer dependances systeme + outils packaging"
            echo "  check-deps   Verifier les dependances systeme"
            echo "  clean        Nettoyer les artefacts de build"
            echo "  help         Afficher cette aide"
            echo ""
            echo "Dependances systeme requises (GUI native egui/eframe):"
            echo "  Debian/Ubuntu: ${DEB_BUILD_DEPS[*]}"
            echo "  RHEL/Fedora:   ${RPM_BUILD_DEPS[*]}"
            ;;
        *)
            error "Commande inconnue: ${cmd}"
            echo "Lancez '$0 help' pour l'aide"
            exit 1
            ;;
    esac
}

main "$@"
