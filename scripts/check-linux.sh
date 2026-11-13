#!/bin/bash
# Runs the Rust tests and the PortMaster checks inside Debian 12, the OS the
# handhelds and CI run, so nothing is skipped for being on a Mac: LuaJIT (the
# shim tests), shellcheck on the generated PortMaster launch script, git (the
# package installer tests), and the same on linux/arm64, which is what a
# PortMaster handheld is. Modelled on r3i6nos's scripts/check-linux.sh.
# Needs Docker (on the maintainer's Mac: `colima start`).
#
#   scripts/check-linux.sh                 native architecture
#   scripts/check-linux.sh --arm64         linux/arm64 (emulated on an Intel Mac)
#   scripts/check-linux.sh --rebuild       rebuild the image first
#
# Not run when written (2026-11-13): the build sandbox cannot reach Docker. Run
# it and fix what it finds. The repo is copied in (without target/ folders), so
# nothing the check writes reaches this checkout.
set -eu
cd "$(dirname "$0")/.."

PLATFORM=()
TAG=native
REBUILD=0
for a in "$@"; do
    case "$a" in
        --arm64) PLATFORM=(--platform linux/arm64); TAG=arm64 ;;
        --rebuild) REBUILD=1 ;;
        *) echo "check-linux: unknown option $a" >&2; exit 2 ;;
    esac
done
IMAGE="${JOYRIDE_CHECK_IMAGE:-joyride-check:bookworm-2-$TAG}"      # bump the tag when the package list changes
command -v docker >/dev/null || { echo "check-linux: needs docker (colima start)" >&2; exit 2; }
docker info >/dev/null 2>&1 || { echo "check-linux: docker is not running (colima start)" >&2; exit 2; }

if [ "$REBUILD" = 1 ] || ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
    echo "[*] Building $IMAGE"
    docker build ${PLATFORM[@]+"${PLATFORM[@]}"} -t "$IMAGE" - <<'DOCKERFILE'
FROM rust:1-bookworm
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
        git luajit shellcheck bash python3 nodejs ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && rustup component add clippy
DOCKERFILE
fi

docker run --rm ${PLATFORM[@]+"${PLATFORM[@]}"} -v "$PWD":/src:ro "$IMAGE" bash -euc '
    mkdir /work && cd /src && tar --exclude=target --exclude=node_modules --exclude=.git -cf - . | tar -C /work -xf -
    cd /work
    echo "[*] $(uname -m): asset-core"; cargo test --manifest-path asset-core/Cargo.toml
    echo "[*] $(uname -m): joyride-core"; cargo test --manifest-path joyride-core/Cargo.toml
    echo "[*] clippy"; cargo clippy --all-targets --manifest-path joyride-core/Cargo.toml -- -D warnings
    echo "[*] shellcheck: the generated PortMaster launch script"
    cargo run -q --manifest-path joyride-core/Cargo.toml --example launch_script > /tmp/launch.sh
    shellcheck -s bash -S warning -e SC1090,SC1091,SC2154,SC2034 /tmp/launch.sh
    echo "[*] PortMaster check script tests"; python3 scripts/test_check_portmaster.py
'
echo "[*] check-linux ($TAG) passed."
