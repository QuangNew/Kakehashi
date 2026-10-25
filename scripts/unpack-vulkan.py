"""Extract only headers, glslc and the loader import library from the signed SDK.

Qt Installer Framework embeds independent 7z archives. Read their standard 7z
start headers instead of running the system installer. Inputs are hash-pinned
by setup-vulkan.ps1. Nothing is installed outside .native-tools.
"""
import mmap
from pathlib import Path
import struct
import subprocess
import sys
import zlib


def main():
    sdk, seven_zip, destination = (Path(arg).resolve() for arg in sys.argv[1:])
    destination.mkdir(parents=True, exist_ok=True)
    targets = {
        "Bin\\glslc.exe": "Bin\\glslc.exe",
        "Include\\vulkan\\vulkan.hpp": "Include\\*",
        "Lib\\vulkan-1.lib": "Lib\\vulkan-1.lib",
    }
    pending = dict(targets)
    archive = sdk.parent / "extract.7z"
    with sdk.open("rb") as source, mmap.mmap(source.fileno(), 0, access=mmap.ACCESS_READ) as data:
        cursor = 0
        while pending:
            offset = data.find(b"7z\xbc\xaf\x27\x1c", cursor)
            if offset < 0:
                raise RuntimeError(f"Missing Vulkan SDK components: {list(pending)}")
            cursor = offset + 6
            if offset + 32 > len(data):
                continue
            if zlib.crc32(data[offset + 12:offset + 32]) != struct.unpack_from("<I", data, offset + 8)[0]:
                continue
            next_offset, next_size, _ = struct.unpack_from("<QQI", data, offset + 12)
            size = 32 + next_offset + next_size
            if offset + size > len(data) or size < 1000:
                continue
            with archive.open("wb") as output:
                end = offset + size
                for chunk in range(offset, end, 1024 * 1024):
                    output.write(data[chunk:min(chunk + 1024 * 1024, end)])
            listing = subprocess.run([str(seven_zip), "l", str(archive)], check=True, capture_output=True, text=True, errors="replace").stdout
            selected = [target for target in pending if target in listing]
            if selected:
                includes = ["-i!" + pending[target] for target in selected]
                subprocess.run([str(seven_zip), "x", str(archive), "-o" + str(destination), "-y", *includes], check=True, stdout=subprocess.DEVNULL)
                for target in selected:
                    if not destination.joinpath(*target.split("\\")).is_file():
                        raise RuntimeError(f"Could not extract {target}")
                    del pending[target]
                    print("Prepared", target)
    (destination / "sdk-version.txt").write_text("1.4.363.0\n", encoding="utf-8")


if __name__ == "__main__":
    main()
