#!/usr/bin/env bash
set -euo pipefail

REPO="easel/yars"
BIN_NAME="yars-format"
INSTALL_DIR_DEFAULT="${HOME}/.local/bin"

TMPDIR_INSTALLER=""

cleanup() {
    if [[ -n "${TMPDIR_INSTALLER}" && -d "${TMPDIR_INSTALLER}" ]]; then
        rm -rf "${TMPDIR_INSTALLER}"
    fi
}

trap cleanup EXIT

usage() {
    cat <<EOF
Installer for ${BIN_NAME}.

Usage: install.sh [options]

Options:
  --version <tag>   Install a specific release tag (default: latest)
  --install-dir DIR Install destination directory (default: ${INSTALL_DIR_DEFAULT})
  --force           Reinstall even if the requested version is already present
  --help            Show this help message
EOF
}

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Error: missing required command '$1'." >&2
        exit 1
    }
}

detect_target() {
    local os arch
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m)"

    case "${os}" in
        linux)
            case "${arch}" in
                x86_64) TARGET="x86_64-unknown-linux-gnu"; EXT="tar.gz" ;;
                *)
                    echo "Unsupported architecture '${arch}' on Linux." >&2
                    exit 1
                    ;;
            esac
            ;;
        darwin)
            case "${arch}" in
                x86_64) TARGET="x86_64-apple-darwin"; EXT="tar.gz" ;;
                arm64|aarch64) TARGET="aarch64-apple-darwin"; EXT="tar.gz" ;;
                *)
                    echo "Unsupported architecture '${arch}' on macOS." >&2
                    exit 1
                    ;;
            esac
            ;;
        *)
            echo "Unsupported operating system '${os}'." >&2
            exit 1
            ;;
    esac
}

fetch_latest_tag() {
    curl -sSf "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name":' \
        | head -n 1 \
        | sed -E 's/.*"([^"]+)".*/\1/'
}

current_installed_version() {
    if [[ -x "${INSTALL_PATH}" ]]; then
        "${INSTALL_PATH}" --version 2>/dev/null | awk '{print $2}'
    else
        echo ""
    fi
}

download_and_install() {
    local tag="$1"
    local tmpdir archive url

    archive="${BIN_NAME}-${tag}-${TARGET}.${EXT}"
    url="https://github.com/${REPO}/releases/download/${tag}/${archive}"

    TMPDIR_INSTALLER="$(mktemp -d)"
    tmpdir="${TMPDIR_INSTALLER}"

    echo "Downloading ${url}"
    curl -sSfL -o "${tmpdir}/${archive}" "${url}"

    mkdir -p "${tmpdir}/extract"
    case "${EXT}" in
        tar.gz)
            tar -xzf "${tmpdir}/${archive}" -C "${tmpdir}/extract"
            ;;
        zip)
            unzip -q "${tmpdir}/${archive}" -d "${tmpdir}/extract"
            ;;
        *)
            echo "Unsupported archive format '${EXT}'." >&2
            exit 1
            ;;
    esac

    mkdir -p "${INSTALL_DIR}"
    install -m755 "${tmpdir}/extract/${BIN_NAME}" "${INSTALL_PATH}"
    echo "Installed ${BIN_NAME} to ${INSTALL_PATH}"
    TMPDIR_INSTALLER=""
}

main() {
    require_cmd curl
    require_cmd tar
    require_cmd install

    local version="latest"
    local force=false
    INSTALL_DIR="${YARS_INSTALL_DIR:-${INSTALL_DIR_DEFAULT}}"

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --version)
                version="${2:?Missing value for --version}"
                shift 2
                ;;
            --install-dir)
                INSTALL_DIR="${2:?Missing value for --install-dir}"
                shift 2
                ;;
            --force)
                force=true
                shift
                ;;
            --help|-h)
                usage
                exit 0
                ;;
            *)
                echo "Unknown option: $1" >&2
                usage
                exit 1
                ;;
        esac
    done

    detect_target

    if [[ "${version}" == "latest" ]]; then
        version="$(fetch_latest_tag)"
    fi

    if [[ -z "${version}" ]]; then
        echo "Unable to determine release tag." >&2
        exit 1
    fi

    if [[ "${version}" != "latest" && "${version}" != v* ]]; then
        version="v${version}"
    fi

    INSTALL_PATH="${INSTALL_DIR}/${BIN_NAME}"

    local current requested
    current="$(current_installed_version)"
    requested="${version#v}"

    if [[ -n "${current}" && "${current}" == "${requested}" && "${force}" != "true" ]]; then
        echo "${BIN_NAME} ${version} is already installed at ${INSTALL_PATH} (use --force to reinstall)."
        exit 0
    fi

    download_and_install "${version}"

    echo "Installed version:"
    "${INSTALL_PATH}" --version || true
}

main "$@"
