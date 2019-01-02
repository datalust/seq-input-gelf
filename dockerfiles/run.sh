#!/bin/bash

exec bin/sqelf | bin/seqcli/seqcli ingest --send-failure=continue --json -s $SEQ_ADDRESS
