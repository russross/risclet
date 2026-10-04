#!/bin/sh
set -eu

# Boot payloads come exclusively from the extracted release, including QEMU setup.
if [ "$#" -ne 2 ]; then
    echo "usage: $0 RELEASE_DIRECTORY RISCLET_BINARY" >&2
    exit 2
fi
release=$(realpath "$1")
cp "$2" build/risclet
for command in curl sha256sum fakeroot cpio gzip tar timeout qemu-system-riscv64 mkfs.erofs; do
    command -v "$command" >/dev/null 2>&1 || { echo "missing command: $command" >&2; exit 1; }
done
kernel=$(find "$release" -maxdepth 1 -name 'linux-*.gz')
firmware=$(find "$release" -maxdepth 1 -name 'fw_dynamic.bin-*.gz')
test -f "$kernel" && test -f "$firmware" || { echo "release boot payloads are missing" >&2; exit 1; }

# The pinned minirootfs is verified before its files enter the setup initramfs.
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
output=$(mktemp -d "$(pwd)/build/output.XXXXXX")
trap 'rm -rf "$stage" "$output" build/rootfs.erofs.part' EXIT HUP INT TERM

# Fakeroot preserves the minirootfs's numeric ownership in cpio without host privileges.
gzip -dc "$kernel" > build/linux
gzip -dc "$firmware" > build/fw_dynamic.bin
fakeroot sh -eu -c '
    tar -xpf "$1" -C "$2"
    cp -R "$3/." "$2/"
    cd "$2"
    find . -print0 | cpio --null --quiet -o -H newc | gzip -1 > "$4"
' sh "$archive" "$stage" "$(pwd)/guest" "$(pwd)/build/setup-initramfs.gz"

# QEMU supplies apk networking; ISA fallback accepts older QEMU device trees.
if ! timeout 300 qemu-system-riscv64 \
    -machine virt -m 512M -smp 1 -nographic -no-reboot \
    -bios build/fw_dynamic.bin -kernel build/linux -initrd build/setup-initramfs.gz \
    -append 'console=ttyS0,115200 rdinit=/sbin/demo-prepare riscv_isa_fallback panic=-1' \
    -netdev user,id=net -device virtio-net-device,netdev=net \
    -fsdev "local,id=source,path=$(pwd),security_model=none,readonly=on" \
    -device virtio-9p-device,fsdev=source,mount_tag=source \
    -fsdev "local,id=output,path=$output,security_model=none" \
    -device virtio-9p-device,fsdev=output,mount_tag=output \
    > build/image-setup.log 2>&1; then
    echo 'QEMU image preparation failed; see build/image-setup.log' >&2
    tail -20 build/image-setup.log >&2
    exit 1
fi
if [ ! -f "$output/complete" ] || [ -f "$output/failed" ]; then
    echo 'Guest image preparation failed; see build/image-setup.log' >&2
    tail -20 build/image-setup.log >&2
    exit 1
fi

# Guest tar headers retain root ownership, the demo UID, symlinks, and setuid doas.
mkfs.erofs --tar=f build/rootfs.erofs.part "$output/rootfs.tar"
mv build/rootfs.erofs.part build/rootfs.erofs
echo 'Prepared Alpine EROFS image with the published Risclet binary.'
