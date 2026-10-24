#!/bin/sh
set -eu

usage() {
    cat <<'EOF'
Usage: ./build.sh --endpoint URL --api-key KEY --model MODEL [options]

Required:
  -e, --endpoint URL   Responses API base URL or full /responses URL
  -k, --api-key KEY   API key to embed in the executable
  -m, --model MODEL   Model name to embed in the executable

Options:
  -t, --target TARGET Rust target triple (for example aarch64-unknown-linux-musl)
      --docker        Cross-compile with Docker; no local Rust target/linker needed
      --podman        Cross-compile with Podman; no local Rust target/linker needed
  -o, --output PATH   Copy the executable to this path
  -h, --help          Show this help

Example:
  ./build.sh \
    --endpoint https://api.openai.com/v1 \
    --api-key sk-your-key \
    --model gpt-5-mini \
    --output embedai

Warning: command-line arguments may be visible in shell history and process listings.
EOF
}

ENDPOINT=''
API_KEY=''
MODEL=''
TARGET=''
OUTPUT=''
CONTAINER_ENGINE=''

while [ "$#" -gt 0 ]; do
    case "$1" in
        -e|--endpoint)
            [ "$#" -ge 2 ] || { echo "build.sh: $1 requires a value" >&2; exit 2; }
            ENDPOINT=$2
            shift 2
            ;;
        -k|--api-key)
            [ "$#" -ge 2 ] || { echo "build.sh: $1 requires a value" >&2; exit 2; }
            API_KEY=$2
            shift 2
            ;;
        -m|--model)
            [ "$#" -ge 2 ] || { echo "build.sh: $1 requires a value" >&2; exit 2; }
            MODEL=$2
            shift 2
            ;;
        -t|--target)
            [ "$#" -ge 2 ] || { echo "build.sh: $1 requires a value" >&2; exit 2; }
            TARGET=$2
            shift 2
            ;;
        --docker|--podman)
            CONTAINER_ENGINE=${1#--}
            shift
            ;;
        -o|--output)
            [ "$#" -ge 2 ] || { echo "build.sh: $1 requires a value" >&2; exit 2; }
            OUTPUT=$2
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "build.sh: unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [ -z "$ENDPOINT" ] || [ -z "$API_KEY" ] || [ -z "$MODEL" ]; then
    echo "build.sh: --endpoint, --api-key, and --model are required" >&2
    usage >&2
    exit 2
fi

export OPENAI_API_KEY="$API_KEY"
export OPENAI_BASE_URL="$ENDPOINT"
export OPENAI_MODEL="$MODEL"

if [ -n "$CONTAINER_ENGINE" ] && [ -z "$TARGET" ]; then
    echo "build.sh: --$CONTAINER_ENGINE requires --target" >&2
    exit 2
fi

if [ -n "$CONTAINER_ENGINE" ]; then
    case "$TARGET" in
        x86_64-unknown-linux-musl) IMAGE="messense/rust-musl-cross:x86_64-musl" ;;
        aarch64-unknown-linux-musl) IMAGE="messense/rust-musl-cross:aarch64-musl" ;;
        armv7-unknown-linux-musleabihf) IMAGE="messense/rust-musl-cross:armv7-musleabihf" ;;
        *) IMAGE="ghcr.io/cross-rs/$TARGET:main" ;;
    esac
    "$CONTAINER_ENGINE" run --rm \
        -v "$PWD:/project" \
        -w /project \
        -e OPENAI_API_KEY \
        -e OPENAI_BASE_URL \
        -e OPENAI_MODEL \
        "$IMAGE" \
        cargo build --release --locked --target "$TARGET"
elif [ -n "$TARGET" ]; then
    cargo build --release --locked --target "$TARGET"
else
    cargo build --release --locked
fi

if [ -n "$TARGET" ]; then
    ARTIFACT="target/$TARGET/release/embedai"
    case "$TARGET" in
        *windows*) ARTIFACT="$ARTIFACT.exe" ;;
    esac
else
    ARTIFACT="target/release/embedai"
fi

if [ -n "$OUTPUT" ]; then
    OUTPUT_DIR=$(dirname "$OUTPUT")
    mkdir -p "$OUTPUT_DIR"
    cp "$ARTIFACT" "$OUTPUT"
    chmod +x "$OUTPUT"
    ARTIFACT=$OUTPUT
fi

printf 'Built %s\n' "$ARTIFACT"
