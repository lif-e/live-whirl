#!/bin/bash
# RUST_BACKTRACE='full' cargo build --release --bin=live-whirl --package=live-whirl
RUST_BACKTRACE='full' cargo build --bin=live-whirl --package=live-whirl

DIR="./out"
OUTPUT="./out.mp4"
PIPE="./ffmpegpipe-000001"

rm "$PIPE"

# Create a named pipe
mkfifo "$PIPE"

# Start FFmpeg process reading from the pipe
ffmpeg -f image2pipe -framerate 60 -i "$PIPE" -c:v libx264 -pix_fmt yuv420p -y "$OUTPUT" &
RUST_BACKTRACE='full' ./target/release/live-whirl &


# Function to add frame to pipe and then delete it
add_frame_to_pipe() {
    echo "$1"
    cat "$1" > "$PIPE"
    rm "$1"
}

for FILE in $(ls -rt "$DIR"/*.png)
do
    if [ -f "$FILE" ]
    then
        add_frame_to_pipe "$FILE"
    fi
done
# Monitor directory for new files
fswatch -o "$DIR" | while read num
do
    for FILE in $(ls -rt "$DIR"/*.png)
    do
        if [ -f "$FILE" ]
        then
            add_frame_to_pipe "$FILE"
        fi
    done
done

# Cleanup: Close the pipe and remove it
exec 3>&-
rm "$PIPE"
