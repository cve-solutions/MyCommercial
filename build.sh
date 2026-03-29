#!/usr/bin/env bash
#
# build.sh - Compile MyCommercial (GUI native) et generer les packages .deb et .rpm
#
# Verifie et installe automatiquement toutes les dependances manquantes
# avant de compiler (systeme, cargo-deb, cargo-generate-rpm).
#
# Usage:
#   ./build.sh              # Auto-install deps + Compile + .deb + .rpm
#   ./build.sh build        # Auto-install deps + Compile uniquement
#   ./build.sh deb          # Auto-install deps + Compile + .deb
#   ./build.sh rpm          # Auto-install deps + Compile + .rpm
#   ./build.sh all          # Auto-install deps + Compile + .deb + .rpm
#   ./build.sh clean        # Nettoyer les artefacts
#   ./build.sh help         # Aide
#
set -euo pipefail

# ── Configuration ──
PROJECT_NAME="mycommercial"
ARCH=$(uname -m)
BUILD_DIR="target/release"
DIST_DIR="dist"

# Auto-increment patch version (0.2.0 -> 0.2.1 -> 0.2.2 ...)
bump_version() {
    local current
    current=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
    local major minor patch
    IFS='.' read -r major minor patch <<< "$current"
    patch=$((patch + 1))
    local new_version="${major}.${minor}.${patch}"
    sed -i "s/^version = \"${current}\"/version = \"${new_version}\"/" Cargo.toml
    ok "Version: ${current} -> ${new_version}"
    echo "$new_version"
}

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')

# Couleurs
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${CYAN}[INFO]${NC} $*"; }
ok()    { echo -e "${GREEN}[ OK ]${NC} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
error() { echo -e "${RED}[ERR ]${NC} $*" >&2; }
step()  { echo -e "\n${BOLD}${CYAN}── $* ──${NC}"; }

# ── Dependances systeme egui/eframe (OpenGL + X11/Wayland) ──

DEB_BUILD_DEPS=(
    build-essential pkg-config dpkg-dev liblzma-dev
    libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
    libxkbcommon-dev libfontconfig1-dev libfreetype-dev
    libgl-dev libegl-dev libwayland-dev libxcb1-dev
)

RPM_BUILD_DEPS=(
    gcc make rpm-build pkgconf-pkg-config
    libxcb-devel libxkbcommon-devel fontconfig-devel freetype-devel
    mesa-libGL-devel mesa-libEGL-devel wayland-devel xz-devel
)

DEB_RUNTIME_DEPS="libc6 (>= 2.31), libgl1, libegl1, libfontconfig1, libxcb-render0, libxcb-shape0, libxcb-xfixes0, libxkbcommon0"
RPM_RUNTIME_DEPS="glibc >= 2.17, mesa-libGL, mesa-libEGL, fontconfig, libxcb, libxkbcommon"

# ══════════════════════════════════════════════════
# Auto-detection et installation des dependances
# ══════════════════════════════════════════════════

ensure_rust() {
    if command -v cargo &>/dev/null; then
        ok "Rust $(rustc --version | cut -d' ' -f2) detecte"
        return 0
    fi

    warn "Rust/Cargo non installe. Installation automatique..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env" 2>/dev/null || true
    export PATH="$HOME/.cargo/bin:$PATH"

    if command -v cargo &>/dev/null; then
        ok "Rust $(rustc --version | cut -d' ' -f2) installe avec succes"
    else
        error "Impossible d'installer Rust automatiquement."
        error "Installez manuellement: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
}

detect_pkg_manager() {
    # Use /etc/os-release for reliable distro detection, fallback to command check
    local id=""
    local id_like=""
    if [ -f /etc/os-release ]; then
        id=$(. /etc/os-release && echo "${ID:-}")
        id_like=$(. /etc/os-release && echo "${ID_LIKE:-}")
    fi

    case "$id" in
        fedora|rhel|rocky|alma|centos)
            if command -v dnf &>/dev/null; then echo "dnf"
            elif command -v yum &>/dev/null; then echo "yum"
            else echo "unknown"; fi
            return ;;
        ubuntu|debian|linuxmint|pop)
            if command -v apt-get &>/dev/null; then echo "apt"; else echo "unknown"; fi
            return ;;
        arch|manjaro)
            if command -v pacman &>/dev/null; then echo "pacman"; else echo "unknown"; fi
            return ;;
    esac

    # Fallback: check ID_LIKE
    case "$id_like" in
        *fedora*|*rhel*)
            if command -v dnf &>/dev/null; then echo "dnf"
            elif command -v yum &>/dev/null; then echo "yum"
            else echo "unknown"; fi
            return ;;
        *debian*|*ubuntu*)
            if command -v apt-get &>/dev/null; then echo "apt"; else echo "unknown"; fi
            return ;;
        *arch*)
            if command -v pacman &>/dev/null; then echo "pacman"; else echo "unknown"; fi
            return ;;
    esac

    # Last resort: command existence check (dnf before apt to avoid false positive)
    if command -v dnf &>/dev/null; then
        echo "dnf"
    elif command -v yum &>/dev/null; then
        echo "yum"
    elif command -v apt-get &>/dev/null; then
        echo "apt"
    elif command -v pacman &>/dev/null; then
        echo "pacman"
    else
        echo "unknown"
    fi
}

ensure_system_deps() {
    local pkg_mgr
    pkg_mgr=$(detect_pkg_manager)
    local missing=()

    case "$pkg_mgr" in
        apt)
            for pkg in "${DEB_BUILD_DEPS[@]}"; do
                if ! dpkg -s "$pkg" &>/dev/null 2>&1; then
                    missing+=("$pkg")
                fi
            done
            if [ ${#missing[@]} -gt 0 ]; then
                info "Installation de ${#missing[@]} dependance(s) manquante(s): ${missing[*]}"
                sudo apt-get update -qq
                sudo apt-get install -y -qq "${missing[@]}"
                ok "Dependances systeme installees"
            else
                ok "Dependances systeme OK"
            fi
            ;;
        dnf)
            for pkg in "${RPM_BUILD_DEPS[@]}"; do
                if ! rpm -q "$pkg" &>/dev/null 2>&1; then
                    missing+=("$pkg")
                fi
            done
            if [ ${#missing[@]} -gt 0 ]; then
                info "Installation de ${#missing[@]} dependance(s) manquante(s): ${missing[*]}"
                sudo dnf install -y "${missing[@]}"
                ok "Dependances systeme installees"
            else
                ok "Dependances systeme OK"
            fi
            ;;
        yum)
            for pkg in "${RPM_BUILD_DEPS[@]}"; do
                if ! rpm -q "$pkg" &>/dev/null 2>&1; then
                    missing+=("$pkg")
                fi
            done
            if [ ${#missing[@]} -gt 0 ]; then
                info "Installation de ${#missing[@]} dependance(s) manquante(s): ${missing[*]}"
                sudo yum install -y "${missing[@]}"
                ok "Dependances systeme installees"
            else
                ok "Dependances systeme OK"
            fi
            ;;
        pacman)
            # Arch Linux equivalents
            local ARCH_DEPS=(base-devel pkgconf libxcb libxkbcommon fontconfig freetype2 mesa wayland)
            for pkg in "${ARCH_DEPS[@]}"; do
                if ! pacman -Qi "$pkg" &>/dev/null 2>&1; then
                    missing+=("$pkg")
                fi
            done
            if [ ${#missing[@]} -gt 0 ]; then
                info "Installation de ${#missing[@]} dependance(s) manquante(s): ${missing[*]}"
                sudo pacman -S --noconfirm "${missing[@]}"
                ok "Dependances systeme installees"
            else
                ok "Dependances systeme OK"
            fi
            ;;
        *)
            warn "Gestionnaire de paquets non reconnu."
            warn "Assurez-vous d'avoir installe: OpenGL dev, X11/XCB dev, fontconfig dev, libxkbcommon dev"
            ;;
    esac
}

ensure_cargo_deb() {
    if command -v cargo-deb &>/dev/null; then
        ok "cargo-deb disponible"
        return 0
    fi

    info "Installation de cargo-deb..."
    if cargo install cargo-deb 2>&1 | tail -1; then
        ok "cargo-deb installe"
        return 0
    else
        warn "Echec installation cargo-deb (la generation .deb sera ignoree)"
        return 1
    fi
}

ensure_cargo_rpm() {
    if command -v cargo-generate-rpm &>/dev/null; then
        ok "cargo-generate-rpm disponible"
        return 0
    fi

    info "Installation de cargo-generate-rpm..."
    if cargo install cargo-generate-rpm 2>&1 | tail -1; then
        ok "cargo-generate-rpm installe"
        return 0
    else
        warn "Echec installation cargo-generate-rpm (la generation .rpm sera ignoree)"
        return 1
    fi
}

# ══════════════════════════════════════════════════
# Etapes de build
# ══════════════════════════════════════════════════

ensure_all_deps() {
    step "Verification des prerequis"
    ensure_rust
    ensure_system_deps
}

build_release() {
    step "Increment version"
    VERSION=$(bump_version)

    step "Compilation release v${VERSION}"
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

    # Copy to dist/
    mkdir -p "${DIST_DIR}"
    local dist_name="${PROJECT_NAME}-v${VERSION}-${ARCH}-linux"
    cp "${BUILD_DIR}/${PROJECT_NAME}" "${DIST_DIR}/${dist_name}"
    ok "Copie: ${DIST_DIR}/${dist_name}"
}

build_deb() {
    step "Generation du package .deb"
    if ! ensure_cargo_deb; then
        return 1
    fi

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

build_rpm() {
    step "Generation du package .rpm"
    if ! ensure_cargo_rpm; then
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

clean() {
    step "Nettoyage"
    cargo clean 2>/dev/null || true
    rm -rf "${DIST_DIR}"
    ok "Nettoyage termine"
}

summary() {
    echo ""
    echo -e "${GREEN}═══════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}  MyCommercial v${VERSION} (GUI native) - Build termine !${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════════════════${NC}"
    echo ""

    if [ -d "${DIST_DIR}" ]; then
        info "Packages disponibles dans ${DIST_DIR}/:"
        ls -lh "${DIST_DIR}"/ 2>/dev/null || true
    fi

    echo ""
    info "Binaire standalone: ${BUILD_DIR}/${PROJECT_NAME}"
    echo ""
    info "Dependances runtime incluses dans les packages:"
    info "  .deb: ${DEB_RUNTIME_DEPS}"
    info "  .rpm: ${RPM_RUNTIME_DEPS}"
    echo ""
}

# ══════════════════════════════════════════════════
# Main
# ══════════════════════════════════════════════════

main() {
    local cmd="${1:-all}"

    echo -e "${CYAN}╔═══════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║  MyCommercial v${VERSION} - Build System (GUI native)       ║${NC}"
    echo -e "${CYAN}╚═══════════════════════════════════════════════════════╝${NC}"

    case "${cmd}" in
        build)
            ensure_all_deps
            build_release
            ;;
        deb)
            ensure_all_deps
            build_release
            build_deb
            summary
            ;;
        rpm)
            ensure_all_deps
            build_release
            build_rpm
            summary
            ;;
        all|"")
            ensure_all_deps
            build_release
            echo ""
            build_deb || warn "Skipping .deb"
            echo ""
            build_rpm || warn "Skipping .rpm"
            summary
            ;;
        clean)
            clean
            ;;
        help|-h|--help)
            echo "Usage: $0 [commande]"
            echo ""
            echo "Commandes:"
            echo "  build   Compiler le projet en mode release"
            echo "  deb     Compiler + generer le package .deb"
            echo "  rpm     Compiler + generer le package .rpm"
            echo "  all     Compiler + generer .deb et .rpm (defaut)"
            echo "  clean   Nettoyer les artefacts de build"
            echo "  help    Afficher cette aide"
            echo ""
            echo "Le script verifie et installe automatiquement :"
            echo "  - Rust/Cargo (via rustup)"
            echo "  - Dependances systeme (OpenGL, X11, Wayland, fontconfig...)"
            echo "  - cargo-deb (pour .deb)"
            echo "  - cargo-generate-rpm (pour .rpm)"
            echo ""
            echo "Distributions supportees : Debian/Ubuntu, Fedora/RHEL/Rocky, Arch Linux"
            ;;
        *)
            error "Commande inconnue: ${cmd}"
            echo "Lancez '$0 help' pour l'aide"
            exit 1
            ;;
    esac
}

main "$@"
