#!/bin/bash
# Script to build and package Cola for production distribution.
set -e

# Setup Rust Environment variables (since we installed Rust locally in the workspace)
export RUSTUP_HOME="/home/elav/Developer/Cola/.rustup"
export CARGO_HOME="/home/elav/Developer/Cola/.cargo"
export PATH="/home/elav/Developer/Cola/.cargo/bin:$PATH"

echo "🥤 Cola - Build & Distribution Packager"
echo "======================================="

# 1. Compile in static mode using musl target for maximum glibc compatibility
echo "⚙️  Compiling statically with target x86_64-unknown-linux-musl..."
cargo build --release --target x86_64-unknown-linux-musl

# 2. Create distribution structure
DIST_DIR="cola-dist"
echo "📁 Preparing distribution folder '$DIST_DIR'..."
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

# 3. Copy compiled static binary
cp target/x86_64-unknown-linux-musl/release/cola "$DIST_DIR/cola"
chmod +x "$DIST_DIR/cola"

# 4. Generate download_geoip.sh script
cat << 'EOF' > "$DIST_DIR/download_geoip.sh"
#!/bin/bash
# Script to download a GeoLite2-Country.mmdb database to the default Cola path.
set -e

DB_DIR="$HOME/.cola"
DB_PATH="$DB_DIR/GeoLite2-Country.mmdb"

echo "🥤 Cola - GeoIP Database Downloader"
echo "==================================="

read -p "Do you have a MaxMind License Key? (y/N): " HAS_KEY

if [[ "$HAS_KEY" =~ ^[Yy]$ ]]; then
    read -p "Enter your MaxMind License Key: " LICENSE_KEY
    if [ -z "$LICENSE_KEY" ]; then
        echo "❌ License key cannot be empty."
        exit 1
    fi
    DOWNLOAD_URL="https://download.maxmind.com/app/geoip_download?edition_id=GeoLite2-Country&license_key=${LICENSE_KEY}&suffix=tar.gz"
    echo "📥 Downloading official MaxMind GeoLite2 Country database..."
    mkdir -p "$DB_DIR"
    curl -sSL "$DOWNLOAD_URL" | tar -xz -C "$DB_DIR" --strip-components=1 --wildcards '*.mmdb' 2>/dev/null || \
    curl -sSL "$DOWNLOAD_URL" | tar -xz -C "$DB_DIR" --strip-components=1
    mv "$DB_DIR"/*.mmdb "$DB_PATH" 2>/dev/null || true
else
    echo "📥 Downloading latest GeoLite2-Country mirror from public registry..."
    mkdir -p "$DB_DIR"
    curl -sSL -o "$DB_PATH" "https://github.com/P3TERX/GeoLite.mmdb/raw/download/GeoLite2-Country.mmdb" || \
    curl -sSL -o "$DB_PATH" "https://raw.githubusercontent.com/Loyalsoldier/geoip/release/Country.mmdb"
fi

if [ -f "$DB_PATH" ]; then
    echo "✅ Success! GeoIP database installed to: $DB_PATH"
else
    echo "❌ Failed to download database."
    exit 1
fi
EOF

chmod +x "$DIST_DIR/download_geoip.sh"

# 5. Copy README.md
cp README.md "$DIST_DIR/README.md"

# 6. Compress the distribution folder
echo "📦 Packaging everything into 'cola-dist.tar.gz'..."
tar -czf cola-dist.tar.gz "$DIST_DIR"

# 7. Clean up temporary directory
rm -rf "$DIST_DIR"

echo "======================================="
echo "✅ Finished successfully!"
echo "🎁 Created compressed distribution file: cola-dist.tar.gz"
echo "👉 Copy 'cola-dist.tar.gz' to your server, extract it, and run!"
