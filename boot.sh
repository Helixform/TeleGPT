#!/bin/bash

cd "$(dirname "$0")" || exit 1

PWD="$(pwd)"

_BIN_PATH="$PWD/target/release/telegpt"
_CONFIG_PATH="$PWD/config.json"

if [ -n "$TELEGPT_CONFIG" ]; then
    _CONFIG_PATH="$TELEGPT_CONFIG"
fi

exec "$_BIN_PATH" -c "$_CONFIG_PATH"
