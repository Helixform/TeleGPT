#!/bin/sh

cd "$(dirname "$0")" || exit 1

PWD="$(pwd)"

_BIN_PATH="$PWD/telegpt"
_CONFIG_PATH="$PWD/config.json"

if [ -n "$TELEGPT_CONFIG" ]; then
    _CONFIG_PATH="$TELEGPT_CONFIG"
fi

exec "$_BIN_PATH" -c "$_CONFIG_PATH"
