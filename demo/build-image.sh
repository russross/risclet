#!/bin/sh
set -eu

# Image construction needs only the verified minirootfs and published binary.
if [ "$#" -ne 1 ]; then
    echo "usage: $0 RISCLET_BINARY" >&2
    exit 2
fi
binary=$(realpath "$1")
PATH="$PATH:/usr/sbin:/sbin"
export PATH
for command in curl sha256sum fakeroot tar truncate mkfs.ext4; do
    command -v "$command" >/dev/null 2>&1 || { echo "missing command: $command" >&2; exit 1; }
done

# The pinned archive is verified before extraction into the filesystem tree.
version=3.24.2
archive="$(pwd)/build/downloads/alpine-minirootfs-$version-riscv64.tar.gz"
mkdir -p build/downloads
if [ ! -f "$archive" ]; then
    curl --fail --location --show-error --output "$archive.part" \
        "https://dl-cdn.alpinelinux.org/alpine/v${version%.*}/releases/riscv64/$(basename "$archive")"
    mv "$archive.part" "$archive"
fi
printf '%s  %s\n' 57132e6e4f3a4ba9ffdf24e485513ce507e45471e7fd565aeed91c055cd63f7b "$archive" | sha256sum --check --status || {
    echo "minirootfs checksum failed: $archive" >&2
    exit 1
}
stage=$(mktemp -d "$(pwd)/build/rootfs.XXXXXX")
trap 'rm -rf "$stage" build/rootfs.ext4.part' EXIT HUP INT TERM

# One fakeroot session retains numeric ownership through ext4 population.
fakeroot sh -eu -c '
    tar -xpf "$1" -C "$2"
    cp -R "$3/." "$2/"
    cd "$2"
    mkdir -p dev/pts run proc sys tmp mnt usr/local/bin home/risclet
    chmod 1777 tmp
    cp "$4" usr/local/bin/risclet
    chmod 755 usr/local/bin/risclet
    chown -R 0:0 .

    # The login account owns the workspace; root remains locked in minirootfs.
    printf "risclet:x:1000:1000:Risclet:/home/risclet:/bin/sh\n" >> etc/passwd
    printf "risclet:x:1000:\n" >> etc/group
    printf "risclet:::0:::::\n" >> etc/shadow
    chown 1000:1000 home/risclet
    printf "/dev/vda / ext4 rw,noatime 0 0\n" > etc/fstab

    # Populate a fixed-size writable disk without mounting or booting it.
    truncate -s 16M "$5"
    mkfs.ext4 -q -F -m 0 -d . "$5"
' sh "$archive" "$stage" "$(pwd)/guest" "$binary" "$(pwd)/build/rootfs.ext4.part"
mv build/rootfs.ext4.part build/rootfs.ext4
echo 'Prepared 16 MiB Alpine ext4 image with the published Risclet binary.'
