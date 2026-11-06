#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 Jeff DaSilva
"""Independently check a disposable zlib installation and use its exact DSO."""

import ctypes
import os
from pathlib import Path
import stat


def require(condition, detail):
    if not condition:
        raise SystemExit(detail)


stage = Path("stage")
prefix = stage / "usr"
libdir = prefix / "lib"
library_name = "libz.so.1.3.2"
regular = {
    "include/zlib.h": (0o644, "zlib.h"),
    "include/zconf.h": (0o644, "zconf.h"),
    "lib/libz.a": (0o644, None),
    "lib/" + library_name: (0o755, library_name),
    "lib/pkgconfig/zlib.pc": (0o644, "zlib.pc"),
    "share/man/man3/zlib.3": (0o644, "zlib.3"),
}
links = {"lib/libz.so", "lib/libz.so.1"}
directories = {"include", "lib", "lib/pkgconfig", "share", "share/man", "share/man/man3"}
require(not stage.is_symlink() and set(stage.iterdir()) == {prefix}, "invalid stage root")
require(prefix.is_dir() and not prefix.is_symlink(), "invalid install prefix")
expected_paths = set(regular) | links | directories
require({str(path.relative_to(prefix)) for path in prefix.rglob("*")} == expected_paths,
        "unexpected staged path set")
for name in directories:
    path = prefix / name
    require(stat.S_ISDIR(path.lstat().st_mode), "invalid staged directory " + name)
for name, (mode, source) in regular.items():
    path = prefix / name
    metadata = path.lstat()
    require(stat.S_ISREG(metadata.st_mode) and stat.S_IMODE(metadata.st_mode) == mode,
            "invalid staged regular file mode " + name)
    if source is not None:
        require(path.read_bytes() == Path(source).read_bytes(), "incorrect installed bytes " + name)
for name in links:
    path = prefix / name
    require(path.is_symlink() and os.readlink(path) == library_name, "invalid relative link " + name)
require((libdir / "libz.a").read_bytes().startswith(b"!<arch>\n"), "invalid staged archive")
pc = (libdir / "pkgconfig/zlib.pc").read_bytes().splitlines()
require(b"prefix=/usr" in pc and b"Version: 1.3.2" in pc, "invalid pkg-config metadata")

library = libdir / library_name
elf = library.read_bytes()
require(elf.startswith(b"\x7fELF") and elf[5] in (1, 2)
        and int.from_bytes(elf[16:18], "little" if elf[5] == 1 else "big") == 3,
        "invalid staged shared object")
zlib = ctypes.CDLL(str(library.resolve(strict=True)))
zlib.zlibVersion.argtypes = []
zlib.zlibVersion.restype = ctypes.c_char_p
version = zlib.zlibVersion()
header = (prefix / "include/zlib.h").read_bytes()
require(b'#define ZLIB_VERSION "' + version + b'"' in header.splitlines(),
        "installed header and shared object disagree")

length = ctypes.c_ulong
pointer = ctypes.POINTER(length)
zlib.compressBound.argtypes = [length]
zlib.compressBound.restype = length
zlib.compress2.argtypes = [ctypes.c_void_p, pointer, ctypes.c_void_p, length, ctypes.c_int]
zlib.compress2.restype = ctypes.c_int
zlib.uncompress.argtypes = [ctypes.c_void_p, pointer, ctypes.c_void_p, length]
zlib.uncompress.restype = ctypes.c_int
payload = b"staged zlib round trip\x00\xff\n" * 32
source = ctypes.create_string_buffer(payload)
compressed_size = length(zlib.compressBound(len(payload)))
compressed = ctypes.create_string_buffer(compressed_size.value)
require(zlib.compress2(compressed, ctypes.byref(compressed_size), source, len(payload), 6) == 0,
        "compression failed")
restored = ctypes.create_string_buffer(len(payload))
restored_size = length(len(payload))
require(zlib.uncompress(restored, ctypes.byref(restored_size), compressed, compressed_size.value) == 0,
        "decompression failed")
require(restored_size.value == len(payload) and restored.raw == payload, "round-trip data mismatch")
print("zlib=" + version.decode("ascii"))
print("roundtrip=ok")
