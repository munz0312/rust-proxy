#!/bin/bash

for port in 9001 9002 9003; do
    node -e "const p=$port; require('http').createServer((_, r) => r.end('port '+p+'\n')).listen(p)" &
    echo "started backend on $port"
done

trap $'kill $(jobs -p) 2>/dev/null' EXIT
wait
