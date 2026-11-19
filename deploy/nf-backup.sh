#!/usr/bin/env bash
# nf-backup: daily consistent backup of NEXUS FABRIC. The service keeps running.
# Install: install -m 0755 deploy/nf-backup.sh /usr/local/sbin/nf-backup (needs sqlite3). Set NF_BACKUP_DIR to change the destination.
# SQLite databases are copied with the online .backup API; other data-dir files
# (blobs) are copied as-is; master.key and nf-server.service are included.
set -euo pipefail
umask 077

DATA_DIR=/var/lib/nexus-fabric
KEY_FILE=/etc/nexus-fabric/master.key
UNIT_FILE=/etc/systemd/system/nf-server.service
DEST="${NF_BACKUP_DIR:-/root/backups/nexus-fabric}"
KEEP=14

log() { echo "nf-backup: $*"; }
die() { echo "nf-backup: ERROR: $*" >&2; exit 1; }

command -v sqlite3 >/dev/null || die "sqlite3 missing (apt-get install -y sqlite3)"
for f in "$DATA_DIR" "$KEY_FILE" "$UNIT_FILE"; do
  [ -e "$f" ] || die "missing $f"
done

if command -v zstd >/dev/null 2>&1; then EXT=tar.zst; else EXT=tar.gz; fi

install -d -m 0700 "$DEST"
TS=$(date +%Y%m%d-%H%M%S)
NAME="nexus-fabric-$TS.$EXT"
ARCHIVE="$DEST/$NAME"
PART="$DEST/.$NAME.partial"
STAGE=$(mktemp -d "$DEST/.stage.XXXXXX")
trap 'rm -rf "$STAGE" "$PART"' EXIT

mkdir -p "$STAGE/data" "$STAGE/etc/nexus-fabric" "$STAGE/etc/systemd/system"

# 1. SQLite databases: consistent online snapshot, integrity-checked.
shopt -s nullglob
dbs=0
for db in "$DATA_DIR"/*.sqlite "$DATA_DIR"/*.sqlite3 "$DATA_DIR"/*.db; do
  name=$(basename "$db")
  snap="$STAGE/data/$name"
  sqlite3 -cmd '.timeout 30000' "$db" ".backup '$snap'"
  # Snapshot is stored in rollback-journal mode so no -wal/-shm is archived.
  sqlite3 "$snap" "PRAGMA journal_mode=DELETE;" >/dev/null
  rm -f "$snap-wal" "$snap-shm"
  res=$(sqlite3 "$snap" "PRAGMA integrity_check;")
  [ "$res" = "ok" ] || die "integrity_check failed for $name: $res"
  dbs=$((dbs + 1))
  log "snapshot $name integrity ok"
done
[ "$dbs" -gt 0 ] || die "no SQLite database found in $DATA_DIR"

# 2. Everything else in the data dir (blobs/, any other files), live DB files excluded.
find "$DATA_DIR" -mindepth 1 -maxdepth 1 \
  ! -name '*.sqlite*' ! -name '*.sqlite3*' ! -name '*.db*' \
  -exec cp -a {} "$STAGE/data/" \;

# 3. Key and service unit (key copied with cp -p: keeps 0440 root:nexusfabric; never printed).
cp -p "$KEY_FILE" "$STAGE/etc/nexus-fabric/master.key"
cp -p "$UNIT_FILE" "$STAGE/etc/systemd/system/nf-server.service"

# 4. Archive, then verify the compressed stream reads back completely.
if [ "$EXT" = tar.zst ]; then
  tar -C "$STAGE" -cf - data etc | zstd -q -T0 -10 -o "$PART"
  zstd -dc "$PART" | tar -tf - >/dev/null
else
  tar -C "$STAGE" -cf - data etc | gzip -6 > "$PART"
  gzip -dc "$PART" | tar -tf - >/dev/null
fi
mv -f "$PART" "$ARCHIVE"

# 5. sha256 alongside the archive, verified.
( cd "$DEST" && sha256sum "$NAME" > "$NAME.sha256" && sha256sum -c --status "$NAME.sha256" )

# 6. Retention: keep the KEEP newest archives (name carries the timestamp, so sort = age).
mapfile -t all < <(find "$DEST" -maxdepth 1 -type f \
  -regextype posix-extended -regex '.*/nexus-fabric-[0-9]{8}-[0-9]{6}\.tar\.(zst|gz)' \
  -printf '%f\n' | sort)
count=${#all[@]}
if [ "$count" -gt "$KEEP" ]; then
  for ((i = 0; i < count - KEEP; i++)); do
    rm -f -- "$DEST/${all[$i]}" "$DEST/${all[$i]}.sha256"
    log "pruned ${all[$i]}"
  done
fi

size=$(stat -c %s "$ARCHIVE")
sha=$(cut -d' ' -f1 "$DEST/$NAME.sha256")
log "archive $NAME size $size bytes ($(numfmt --to=iec "$size")) sha256 $sha"
log "done: $dbs database(s), archives kept $(( count > KEEP ? KEEP : count ))"

# 7. Optional offsite copy: not configured. To use one, copy the archive and its .sha256 to
#    storage you control. Encrypt first (the archive holds the master key) and keep the passphrase
#    and remote credentials outside this file, for example:
#    gpg --batch --yes --pinentry-mode loopback --passphrase-file /root/.config/nf-backup/passphrase \
#      --symmetric --cipher-algo AES256 -o "$ARCHIVE.gpg" "$ARCHIVE"
#    rclone copyto "$ARCHIVE.gpg" "<remote>:<path>/$NAME.gpg"
