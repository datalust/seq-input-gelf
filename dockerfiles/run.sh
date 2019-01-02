#!/bin/bash

# Add an API Key if specified
api_key_arg=
if [ $SEQ_API_KEY ]; then
    api_key_arg="-a $SEQ_API_KEY"
fi

exec bin/sqelf | bin/seqcli/seqcli ingest --send-failure=continue --json -s $SEQ_ADDRESS $api_key_arg
