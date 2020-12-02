#!/usr/bin/env python3

import sys

# see solidity docs for the swarm hash
# https://solidity.readthedocs.io/en/v0.4.24/metadata.html#encoding-of-the-metadata-hash-in-the-bytecode
# however, this changed quite a lot:
# https://docs.soliditylang.org/en/v0.8.9/metadata.html#encoding-of-the-metadata-hash-in-the-bytecode
# locating the metadata is apparently pretty tricky:
# https://ethereum.stackexchange.com/questions/110991/how-to-verify-smart-contracts-on-different-solidity-versions/111001#111001

signatures = [
    b'\xa1\x65bzzr0\x58\x20',
    b'\xa2\x65bzzr1\x58\x20',
    b'\xa2\x64ipfs\x58\x22',
]


def filter_bbs(bbs, metadata_offsets):
    minoff = min(metadata_offsets)

    return set(bb for bb in bbs if bb < minoff)


def find_all(a_str, sub):
    start = 0

    while True:
        start = a_str.find(sub, start)

        if start == -1:
            return
        yield start
        start += len(sub)


def find_metadata(bytecode):
    ml = []

    for sig in signatures:
        ml.extend(find_all(bytecode, sig))

    return set(ml)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("usage:",
              sys.argv[0],
              "<bytecode_file>",
              "<bb_list_file>",
              """
    This script removes all basic block addresses from the given bb_list
    file, that are located after some form of metadata in the original
    EVM bytecode (bin-runtime).
""",
              file=sys.stderr)
        sys.exit(-1)

    bytecode_path = sys.argv[1]
    bb_list_path = sys.argv[2]

    with open(bytecode_path, "rb") as fb:
        bytecode = fb.read()

    try:
        if bytecode.startswith(b"0x"):
            bytecode = bytes.fromhex(bytecode[2:].decode('ascii'))
        else:
            bytecode = bytes.fromhex(bytecode.decode('ascii'))
    except ValueError:
        # assuming it is binary already
        pass

    with open(bb_list_path) as f:
        bbs = set(int(s.strip(), 0) for s in f.readlines() if s.strip())

    ml = find_metadata(bytecode)
    bbs_new = filter_bbs(bbs, ml)

    print("filtered", len(bbs) - len(bbs_new), "BB locs")

    with open(bb_list_path, "w") as f:
        f.write("\n".join(map(hex, bbs_new)))
