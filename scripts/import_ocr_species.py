"""Extract factual base species/type data from a local B/W NDS ROM and Chinese strings.

Does not copy ROM artwork, executable code, dialogue, or save data.
"""
import argparse
from pathlib import Path
import struct
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
BW_TYPES = 'normal fighting flying poison ground rock bug ghost steel fire water grass electric psychic ice dragon dark'.split()


def personal_records(rom):
    with rom.open('rb') as file:
        header = file.read(0x50)
        if header[12:15] not in (b'IRA', b'IRB'):
            raise ValueError('Expected Pokemon Black/White 1 ROM (IRA/IRB game code)')
        fnt_offset, fnt_size, fat_offset, fat_size = struct.unpack_from('<4I', header, 0x40)
        file.seek(fnt_offset)
        fnt = file.read(fnt_size)
        file.seek(fat_offset)
        fat = file.read(fat_size)

        def walk(directory, prefix):
            pos, fid, _ = struct.unpack_from('<IHH', fnt, (directory - 0xF000) * 8)
            while fnt[pos]:
                size = fnt[pos]
                pos += 1
                name = fnt[pos:pos + (size & 127)].decode('ascii')
                pos += size & 127
                if size & 128:
                    child = struct.unpack_from('<H', fnt, pos)[0]
                    pos += 2
                    yield from walk(child, prefix + [name])
                else:
                    yield '/'.join(prefix + [name]), fid
                    fid += 1

        fid = dict(walk(0xF000, []))['a/0/1/6']
        start, end = struct.unpack_from('<II', fat, fid * 8)
        file.seek(start)
        raw = file.read(end - start)
    if raw[:4] != b'NARC':
        raise ValueError('Personal data is not a NARC archive')
    blocks = {}
    pos = struct.unpack_from('<H', raw, 12)[0]
    while pos < len(raw):
        size = struct.unpack_from('<I', raw, pos + 4)[0]
        if size < 8 or pos + size > len(raw):
            raise ValueError('Invalid NARC block')
        blocks[raw[pos:pos + 4]] = raw[pos + 8:pos + size]
        pos += size
    index, contents = blocks[b'BTAF'], blocks[b'GMIF']
    if struct.unpack_from('<H', index)[0] < 650:
        raise ValueError('Incomplete base species table')
    records = {}
    for species in range(1, 650):
        start, end = struct.unpack_from('<II', index, 4 + species * 8)
        data = contents[start:end]
        if len(data) != 60 or max(data[6:8]) >= len(BW_TYPES):
            raise ValueError(f'Unexpected B/W personal record for #{species}')
        records[species] = tuple(BW_TYPES[t] for t in data[6:8])
    # Catch a shifted type enum or wrong archive before generating user-facing data.
    for species, expected in {1: ('grass', 'poison'), 4: ('fire', 'fire'),
                              25: ('electric', 'electric'), 523: ('electric', 'electric'),
                              573: ('normal', 'normal'), 649: ('bug', 'steel')}.items():
        if records[species] != expected:
            raise ValueError(f'Type sanity check failed for #{species}')
    return records


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--rom', type=Path, required=True)
    parser.add_argument('--strings', type=Path, required=True)
    parser.add_argument('--output', type=Path, default=ROOT / 'windows-ocr/data/species.tsv')
    args = parser.parse_args()
    records = personal_records(args.rom)
    names = {int(e.get('id')): e.text for e in ET.parse(args.strings).getroot().findall('string')}
    rows = ['# id\tChinese name\tB/W base type1\tB/W base type2 (empty for monotypes)']
    for species, (first, second) in records.items():
        name = names.get(150000 + species)
        if not name or any(c in name for c in '\t\n[]'):
            raise ValueError(f'Missing or annotated name for #{species}')
        rows.append(f'{species}\t{name}\t{first}\t{second if second != first else ""}')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text('\n'.join(rows) + '\n', encoding='utf-8')
    print(f'Wrote {len(records)} base species to {args.output}')
