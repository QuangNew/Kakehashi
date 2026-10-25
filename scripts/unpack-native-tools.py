"""Extract pinned portable build tools inside this workspace."""
from pathlib import Path
from zipfile import ZipFile

root = Path(__file__).resolve().parent.parent / ".native-tools"
for prefix, folder in [("cmake-3.31.6", "cmake"), ("ninja-1.13.2", "ninja"), ("libclang-18.1.1", "libclang")]:
    wheel = next((root / "packages").glob(prefix + "*.whl"))
    destination = root / folder
    with ZipFile(wheel) as archive:
        for member in archive.infolist():
            if not (destination / member.filename).resolve().is_relative_to(destination.resolve()):
                raise ValueError("Archive member leaves the portable tools directory")
        archive.extractall(destination)
