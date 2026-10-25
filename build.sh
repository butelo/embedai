#!/bin/sh
set -eu

usage() {
    cat <<'EOF'
Usage: ./build.sh --config FILE [options]
       ./build.sh --endpoint URL --api-key KEY --model MODEL [options]

Required (unless supplied by --config):
  -e, --endpoint URL   Responses API base URL or full /responses URL
  -k, --api-key KEY   API key to embed in the executable
  -m, --model MODEL   Model name to embed in the executable

Options:
  -c, --config FILE  Read options from JSON; CLI options take precedence
  -t, --target TARGET Rust target triple (for example aarch64-unknown-linux-musl)
      --docker        Cross-compile with Docker; no local Rust target/linker needed
      --podman        Cross-compile with Podman; no local Rust target/linker needed
  -o, --output PATH   Copy the executable to this path
  -h, --help          Show this help

Examples:
  ./build.sh --config build.json

  ./build.sh \
    --endpoint https://api.openai.com/v1 \
    --api-key sk-your-key \
    --model gpt-5-mini \
    --output embedai

JSON config keys: endpoint, api_key, model, target, output, docker, podman.
Using --config requires jq.
Warning: command-line arguments may be visible in shell history and process listings.
EOF
}

ENDPOINT=''
API_KEY=''
MODEL=''
TARGET=''
OUTPUT=''
CONTAINER_ENGINE=''
CONFIG_FILE=''
CONFIG_REQUESTED=false

# Find and load the config first so the regular argument pass can override it.
find_config() {
    while [ "$#" -gt 0 ]; do
        case "$1" in
            -c|--config)
                [ "$#" -ge 2 ] || { echo "build.sh: $1 requires a value" >&2; exit 2; }
                CONFIG_FILE=$2
                CONFIG_REQUESTED=true
                shift 2
                ;;
            --config=*)
                CONFIG_FILE=${1#*=}
                CONFIG_REQUESTED=true
                shift
                ;;
            -e|--endpoint|-k|--api-key|-m|--model|-t|--target|-o|--output)
                # Skip values so an option-like value is not treated as --config.
                if [ "$#" -ge 2 ]; then shift 2; else shift; fi
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                shift
                ;;
        esac
    done
}

load_config() {
    command -v jq >/dev/null 2>&1 || {
        echo 'build.sh: --config requires jq' >&2
        exit 2
    }

    CONFIG_ASSIGNMENTS=$(jq -er '
        def string_value($key):
            if .[$key] == null then ""
            elif (.[$key] | type) != "string" then
                error("config key \($key) must be a string")
            elif (.[$key] | contains("\u0000")) then
                error("config key \($key) cannot contain NUL")
            else .[$key]
            end;

        def bool_value($key):
            if .[$key] == null then false
            elif (.[$key] | type) == "boolean" then .[$key]
            else error("config key \($key) must be true or false")
            end;

        if type != "object" then error("config must be a JSON object") else . end
        | (keys_unsorted - ["endpoint", "api_key", "model", "target", "output", "docker", "podman"]) as $unknown
        | if ($unknown | length) > 0 then
              error("unknown config key(s): \($unknown | join(", "))")
          else . end
        | string_value("endpoint") as $endpoint
        | string_value("api_key") as $api_key
        | string_value("model") as $model
        | string_value("target") as $target
        | string_value("output") as $output
        | bool_value("docker") as $docker
        | bool_value("podman") as $podman
        | if $docker and $podman then error("docker and podman cannot both be true") else . end
        | "ENDPOINT=\($endpoint | @sh)",
          "API_KEY=\($api_key | @sh)",
          "MODEL=\($model | @sh)",
          "TARGET=\($target | @sh)",
          "OUTPUT=\($output | @sh)",
          "CONTAINER_ENGINE=\((if $docker then "docker" elif $podman then "podman" else "" end) | @sh)"
    ' "$1") || exit $?

    eval "$CONFIG_ASSIGNMENTS"
    unset CONFIG_ASSIGNMENTS
}

find_config "$@"
if [ "$CONFIG_REQUESTED" = true ]; then
    [ -n "$CONFIG_FILE" ] || { echo "build.sh: --config requires a value" >&2; exit 2; }
    load_config "$CONFIG_FILE"
fi

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
        -c|--config)
            [ "$#" -ge 2 ] || { echo "build.sh: $1 requires a value" >&2; exit 2; }
            shift 2
            ;;
        --config=*)
            shift
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
