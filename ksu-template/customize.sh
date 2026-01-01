#!/system/bin/sh

DATA_DIR="/data/adb/micetimer"
DATA_BIN="$DATA_DIR/bin"
DATA_TIMERS="$DATA_DIR/timers.d"

ui_print "- Initializing MiceTimer Storage..."
mkdir -p "$DATA_BIN"
mkdir -p "$DATA_TIMERS"

# 1. Move binary from module temp path to persistent storage
# The zip layout has bin/micetimer (the physical file)
if [ -f "$MODPATH/bin/micetimer" ]; then
    ui_print "- Deploying binary..."
    mv -f "$MODPATH/bin/micetimer" "$DATA_BIN/micetimer"
    chmod 755 "$DATA_BIN/micetimer"
    # Remove the physical bin dir in module, leaving only system/bin symlink if it exists (via overlay)
    rm -rf "$MODPATH/bin"
else
    ui_print "❌ Error: Binary not found in package!"
    abort
fi

# 2. Setup permissions and SELinux context
if [ -x "$(command -v chcon)" ]; then
    ui_print "- Setting security context..."
    chcon --reference /system/etc/hosts "$DATA_BIN/micetimer" 2>/dev/null || true
fi

ui_print "✅ Installation Complete."
ui_print "   Config dir: $DATA_TIMERS"
