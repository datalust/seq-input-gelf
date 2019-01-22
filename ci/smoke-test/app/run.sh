#!/bin/bash

# Text written to stdout will be treated as a log event
echo 'This is a plaintext log'

# Text that's already CLEF formatted will be treated as
# structured data
echo '{"@mt": "This is a {kind} log", "kind": "Structured"}'
