#!/bin/bash
set -eo pipefail

# Add an API Key if specified
api_key_arg=
if [ -f "$SEQ_API_KEY_FILE" ]; then
    SEQ_API_KEY=$(cat "$SEQ_API_KEY_FILE")
fi
if [ $SEQ_API_KEY ]; then
    api_key_arg="-a $SEQ_API_KEY"
fi

bin/sqelf | bin/seqcli/seqcli ingest --invalid-data=ignore --send-failure=continue --json -s $SEQ_ADDRESS $api_key_arg
