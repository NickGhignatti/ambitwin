#!/bin/sh
# Runs a Rust service's tests/*.rs inside its compose network (container to container, so no
# host-published port), then removes every container. Run from the service's own directory.
set -eu

service=$(basename "$PWD")

# the base image's compiler comes from the one pin in .mise.toml; nothing else exports it
RUST_VERSION=$(sed -n 's/^rust = "\(.*\)"$/\1/p' "$(dirname "$0")/../.mise.toml")
export RUST_VERSION

compose() { docker compose -p "$service-integration-tests" -f docker-compose.test.yml "$@"; }

# a trap, not a trailing `down`: under `set -e` a failed `up` exits before reaching it
trap 'compose down --remove-orphans --volumes' EXIT

compose up --build --exit-code-from "$service-test" --abort-on-container-exit
