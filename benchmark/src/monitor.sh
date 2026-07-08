#!/bin/bash

if [ -z "$1" ]; then
    echo "Usage: $0 \"tunnel_command --args\""
    exit 1
fi

TUNNEL_CMD=$1

echo "============ BENCHMARK INITIALIZATION ============"
echo "Command: $TUNNEL_CMD"
echo "Press CTRL+C to stop and save monitoring metrics."
echo "--------------------------------------------------"

eval "exec $TUNNEL_CMD" &
MAIN_PID=$!

echo "$MAIN_PID"

sleep 0.5

if ! kill -0 $MAIN_PID 2>/dev/null; then
    echo "Error: Failed to start the tunnel command."
    exit 1
fi

PROCESS_NAME=$(ps -p $MAIN_PID -o comm=)
OUTPUT_FILE="${PROCESS_NAME}_pidstat_metrics.json"

cleanup() {
    kill $PIDSTAT_PID 2>/dev/null

    kill $MAIN_PID 2>/dev/null
    wait $MAIN_PID 2>/dev/null

    echo "--------------------------------------------------"
    echo "Metrics saved to: $OUTPUT_FILE"
    echo "=================================================="
    exit 0
}

trap cleanup SIGINT

pidstat -p $MAIN_PID -u -r 1 -o JSON > "$OUTPUT_FILE" &
PIDSTAT_PID=$!

wait $PIDSTAT_PID 2>/dev/null
