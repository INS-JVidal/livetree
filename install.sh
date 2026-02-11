#!/bin/sh
# install.sh — Download and install the latest livetree binary.
# Usage: curl -sSfL https://raw.githubusercontent.com/INS-JVidal/livetree/main/install.sh | sh
set -e

REPO="INS-JVidal/livetree"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux)  os="unknown-linux-gnu" ;;
        Darwin) os="apple-darwin" ;;
        *)      echo "Error: unsupported OS: $OS" >&2; exit 1 ;;
    esac

    case "$ARCH" in
        x86_64|amd64)   arch="x86_64" ;;
        aarch64|arm64)   arch="aarch64" ;;
        *)               echo "Error: unsupported architecture: $ARCH" >&2; exit 1 ;;
    esac

    TARGET="${arch}-${os}"
}

main() {
    detect_platform

    ARCHIVE="livetree-${TARGET}.tar.gz"
    URL="https://github.com/${REPO}/releases/latest/download/${ARCHIVE}"

    echo "Detected platform: ${TARGET}"
    echo "Downloading ${URL} ..."

    TMP="$(mktemp -d)"
    trap 'rm -rf "$TMP"' EXIT

    curl -sSfL "$URL" -o "${TMP}/${ARCHIVE}"
    tar -xzf "${TMP}/${ARCHIVE}" -C "$TMP"

    mkdir -p "$INSTALL_DIR"
    install -m 0755 "${TMP}/livetree" "${INSTALL_DIR}/livetree"

    echo "Installed livetree to ${INSTALL_DIR}/livetree"

    if "${INSTALL_DIR}/livetree" --version >/dev/null 2>&1; then
        echo "$("${INSTALL_DIR}/livetree" --version)"
    fi

    case ":$PATH:" in
        *":${INSTALL_DIR}:"*) ;;
        *) echo "Note: Add ${INSTALL_DIR} to your PATH if not already present." ;;
    esac
}

main
