#!/usr/bin/env bash
#
# build.sh - Compile MyCommercial et generer les packages .deb et .rpm
#
# Usage:
#   ./build.sh              # Compile + .deb + .rpm
#   ./build.sh build        # Compile uniquement (release)
#   ./build.sh deb          # Compile + .deb uniquement
#   ./build.sh rpm          # Compile + .rpm uniquement
#   ./build.sh all          # Compile + .deb + .rpm
#   ./build.sh install-deps # Installer les outils de packaging
#   ./build.sh clean        # Nettoyer les artefacts
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

# ── Verifier les prerequis ──
check_rust() {
    if ! command -v cargo &>/dev/null; then
        error "Rust/Cargo non installe. Installez via: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
    info "Rust $(rustc --version | cut -d' ' -f2) detecte"
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

# ── Installer les dependances de packaging ──
install_deps() {
    info "Installation des outils de packaging..."

    # Dependances systeme pour cargo-deb
    if command -v apt-get &>/dev/null; then
        info "Systeme Debian/Ubuntu detecte"
        sudo apt-get update -qq
        sudo apt-get install -y -qq dpkg-dev liblzma-dev pkg-config build-essential 2>/dev/null || true
    elif command -v dnf &>/dev/null; then
        info "Systeme Fedora/RHEL detecte"
        sudo dnf install -y rpm-build gcc make 2>/dev/null || true
    elif command -v yum &>/dev/null; then
        info "Systeme CentOS/RHEL detecte"
        sudo yum install -y rpm-build gcc make 2>/dev/null || true
    fi

    # Outils Cargo
    info "Installation de cargo-deb..."
    cargo install cargo-deb 2>/dev/null || warn "Echec installation cargo-deb"

    info "Installation de cargo-generate-rpm..."
    cargo install cargo-generate-rpm 2>/dev/null || warn "Echec installation cargo-generate-rpm"

    ok "Outils de packaging installes"
}

# ── Compilation ──
build_release() {
    info "Compilation en mode release..."
    cargo build --release 2>&1

    if [ ! -f "${BUILD_DIR}/${PROJECT_NAME}" ]; then
        error "Le binaire n'a pas ete genere"
        exit 1
    fi

    # Taille du binaire
    local size
    size=$(du -h "${BUILD_DIR}/${PROJECT_NAME}" | cut -f1)
    ok "Binaire compile: ${BUILD_DIR}/${PROJECT_NAME} (${size})"

    # Strip les symboles de debug pour reduire la taille
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

    info "Generation du package .deb..."
    cargo deb --no-build 2>&1

    # Trouver le .deb genere
    local deb_file
    deb_file=$(find target/debian -name "*.deb" -type f -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2)

    if [ -n "${deb_file}" ]; then
        mkdir -p "${DIST_DIR}"
        cp "${deb_file}" "${DIST_DIR}/"
        local size
        size=$(du -h "${deb_file}" | cut -f1)
        ok "Package .deb genere: ${deb_file} (${size})"

        # Afficher les infos du package
        if command -v dpkg-deb &>/dev/null; then
            echo ""
            info "Contenu du package:"
            dpkg-deb -I "${deb_file}" 2>/dev/null | head -15 || true
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

    # Trouver le .rpm genere
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
    echo -e "${GREEN}═══════════════════════════════════════════════${NC}"
    echo -e "${GREEN}  MyCommercial v${VERSION} - Build termine${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════════${NC}"
    echo ""

    if [ -d "${DIST_DIR}" ]; then
        info "Packages disponibles dans ${DIST_DIR}/:"
        ls -lh "${DIST_DIR}"/ 2>/dev/null || true
    fi

    echo ""
    info "Le binaire standalone est dans: ${BUILD_DIR}/${PROJECT_NAME}"
    echo ""
}

# ── Main ──
main() {
    local cmd="${1:-all}"

    echo -e "${CYAN}╔══════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║  MyCommercial v${VERSION} - Build System          ║${NC}"
    echo -e "${CYAN}╚══════════════════════════════════════════════╝${NC}"
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
            echo "  install-deps Installer les outils de packaging"
            echo "  clean        Nettoyer les artefacts de build"
            echo "  help         Afficher cette aide"
            ;;
        *)
            error "Commande inconnue: ${cmd}"
            echo "Lancez '$0 help' pour l'aide"
            exit 1
            ;;
    esac
}

main "$@"
