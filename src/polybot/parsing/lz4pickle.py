"""Decode a K4os.Compression.LZ4 "pickle" (LZ4Pickler) as used for Polytopia savestates.

Layout (header version 0): byte 0 = flags; bits 0-2 version (0), bits 6-7 size-of-diff code
(0 -> 0 bytes, 1 -> 1, 2 -> 2, 3 -> 4). Then `size-of-diff` little-endian bytes holding
(uncompressed length - compressed length). A diff of 0 means the data is stored uncompressed.
The rest is a raw LZ4 block.
"""


def lz4_block_decode(src: bytes, expected_size: int) -> bytes:
    out = bytearray()
    i = 0
    n = len(src)
    while i < n:
        token = src[i]
        i += 1
        lit = token >> 4
        if lit == 15:
            while True:
                b = src[i]
                i += 1
                lit += b
                if b != 255:
                    break
        out += src[i : i + lit]
        i += lit
        if i >= n:  # last sequence has literals only
            break
        offset = src[i] | (src[i + 1] << 8)
        i += 2
        if offset == 0:
            raise ValueError("invalid LZ4 offset 0")
        mlen = token & 0xF
        if mlen == 15:
            while True:
                b = src[i]
                i += 1
                mlen += b
                if b != 255:
                    break
        mlen += 4
        start = len(out) - offset
        if start < 0:
            raise ValueError("LZ4 offset before start of output")
        for k in range(mlen):  # byte-wise: matches may overlap their own output
            out.append(out[start + k])
    if len(out) != expected_size:
        raise ValueError(f"decoded {len(out)} bytes, expected {expected_size}")
    return bytes(out)


def unpickle(data: bytes) -> bytes:
    header = data[0]
    if header & 0x07:
        raise ValueError(f"unknown pickle version {header & 0x07}")
    code = (header >> 6) & 0x3
    diff_size = 4 if code == 3 else code
    data_offset = 1 + diff_size
    diff = int.from_bytes(data[1:data_offset], "little") if diff_size else 0
    body = data[data_offset:]
    if diff == 0:
        return body
    return lz4_block_decode(body, len(body) + diff)
