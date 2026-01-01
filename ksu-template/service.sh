#!/system/bin/sh
MODDIR=${0%/*}
DAEMON="/data/adb/micetimer/bin/micetimer"
LOG_FILE="/data/adb/micetimer/micetimer.log"

# Wait for boot
until [ "$(getprop sys.boot_completed)" = "1" ]; do sleep 5; done
sleep 10

if [ -x "$DAEMON" ]; then
    # Simple log rotation
    if [ -f "$LOG_FILE" ] && [ $(stat -c%s "$LOG_FILE") -gt 1048576 ]; then
        mv "$LOG_FILE" "$LOG_FILE.old"
    fi

    echo "[$(date)] Service starting..." >> "$LOG_FILE"
    
    # Start in background
    nohup "$DAEMON" >> "$LOG_FILE" 2>&1 &
else
    echo "[$(date)] Error: Daemon not found at $DAEMON" >> "$LOG_FILE"
fi
