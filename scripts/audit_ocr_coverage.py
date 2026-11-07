"""Read-only audit of bundled base species coverage against all five local ROMs.

Only factual counts, type comparisons, header identifiers and form labels are saved.
No ROM code, artwork, dialogue or complete personal records are exported.
"""
import argparse
import csv
import json
from pathlib import Path
import struct
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
TYPES = 'normal fighting flying poison ground rock bug ghost steel fire water grass electric psychic ice dragon dark'.split()
OLD_TYPES = TYPES[:9] + ['unused'] + TYPES[9:]


def nds_archive(path, archive):
    with path.open('rb') as file:
        header = file.read(0x50)
        fnt_offset, fnt_size, fat_offset, fat_size = struct.unpack_from('<4I', header, 0x40)
        file.seek(fnt_offset)
        fnt = file.read(fnt_size)
        file.seek(fat_offset)
        fat = file.read(fat_size)

        def walk(directory, prefix):
            pos, fid, _ = struct.unpack_from('<IHH', fnt, (directory - 0xF000) * 8)
            while fnt[pos]:
                length = fnt[pos]
                pos += 1
                name = fnt[pos:pos + (length & 127)].decode('ascii')
                pos += length & 127
                if length & 128:
                    child = struct.unpack_from('<H', fnt, pos)[0]
                    pos += 2
                    yield from walk(child, prefix + name + '/')
                else:
                    yield prefix + name, fid
                    fid += 1

        fid = dict(walk(0xF000, ''))[archive]
        start, end = struct.unpack_from('<II', fat, fid * 8)
        file.seek(start)
        raw = file.read(end - start)
    if raw[:4] != b'NARC':
        raise ValueError('Expected a NARC archive')
    pos = struct.unpack_from('<H', raw, 12)[0]
    blocks = {}
    while pos < len(raw):
        size = struct.unpack_from('<I', raw, pos + 4)[0]
        if size < 8 or pos + size > len(raw):
            raise ValueError('Invalid NARC block')
        blocks[raw[pos:pos + 4]] = raw[pos + 8:pos + size]
        pos += size
    index, data = blocks[b'BTAF'], blocks[b'GMIF']
    count = struct.unpack_from('<H', index)[0]
    records = [data[start:end] for start, end in
               (struct.unpack_from('<II', index, 4 + i * 8) for i in range(count))]
    return header[12:16].decode('ascii'), records


def types(record, keys):
    return tuple(dict.fromkeys(keys[t] for t in record[6:8]))


def compare(expected, records):
    return [dict(national_id=i, bundled=expected.get(i), rom=list(value))
            for i, value in sorted(records.items()) if list(value) != expected.get(i)]


def audit(rom_dir, strings):
    with (ROOT / 'windows-ocr/data/species.tsv').open(encoding='utf-8') as file:
        rows = list(csv.reader((line for line in file if not line.startswith('#')), delimiter='\t'))
    expected = {int(row[0]): [t for t in row[2:] if t] for row in rows}
    assert len(expected) == len(rows) == 649 and set(expected) == set(range(1, 650))
    result = {'scope': '649 national base species; ROM internal slots are not national numbers',
              'bundled_count': len(expected), 'bundled_missing_ids': [], 'roms': []}
    union = set()
    for name, archive, maximum, keys, size in [
        ('心金.nds', 'a/0/0/2', 493, OLD_TYPES, 44),
        ('白金.nds', 'poketool/personal/pl_personal.narc', 493, OLD_TYPES, 44),
        ('黑白.nds', 'a/0/1/6', 649, TYPES, 60),
    ]:
        code, records = nds_archive(rom_dir / name, archive)
        assert all(len(records[i]) == size for i in range(1, maximum + 1))
        base = {i: types(records[i], keys) for i in range(1, maximum + 1)}
        union.update(base)
        result['roms'].append(dict(file=name, game_code=code, personal_archive=archive,
            personal_slots=len(records), national_base_count=maximum,
            base_range=[1, maximum], type_mismatches=compare(expected, base),
            extra_slots_note='Dummy, egg/placeholder and alternate-form slots; not new national species'))
    for name, code in [('火红.gba', b'BPRE'), ('绿宝石.gba', b'BPEE')]:
        data = (rom_dir / name).read_bytes()
        assert data[0xac:0xb0] == code
        signature = bytes([45, 49, 49, 45, 65, 65, 12, 3])  # Bulbasaur stats/types
        start = data.find(signature)
        assert start >= 0 and data.find(signature, start + 1) == -1
        table = start - 28  # Fixed 28-byte records, slot zero precedes Bulbasaur.
        dex_signature = struct.pack('<251H', *range(1, 252))
        dex = data.find(dex_signature)
        assert dex >= 0 and data.find(dex_signature, dex + 1) == -1
        mapping = struct.unpack_from('<411H', data, dex)
        base = {}
        for internal in range(1, 412):
            if 252 <= internal <= 276:  # Gen III's old Unown placeholders, not Gen IV species.
                continue
            national = mapping[internal - 1]
            assert 1 <= national <= 386 and national not in base
            base[national] = types(data[table + internal * 28:table + (internal + 1) * 28], OLD_TYPES)
        assert set(base) == set(range(1, 387))
        assert base[4] == ('fire',) and base[25] == ('electric',) and base[386] == ('psychic',)
        union.update(base)
        result['roms'].append(dict(file=name, game_code=code.decode(), national_base_count=len(base),
            base_range=[1, 386], type_table_offset=hex(table), dex_mapping_offset=hex(dex),
            type_mismatches=compare(expected, base),
            extra_slots_note='Internal old-Unown slots map above 386 but are not Gen IV national species'))
    entries = {int(e.get('id')): e.text for e in ET.parse(strings).getroot().iter('string')}
    missing_names = [i for i in range(1, 650) if entries.get(150000 + i) != rows[i - 1][1]]
    result['chinese_base_name_mismatches'] = missing_names
    result['union_national_count'] = len(union)
    result['union_missing_from_app'] = sorted(union - expected.keys())
    result['client_extra_labels'] = [dict(string_id=i, label=text)
        for i, text in sorted(entries.items()) if 150650 <= i < 151000]
    result['limitations'] = [
        'Base species coverage is not an assertion that every species is currently catchable in PokeMMO.',
        'Alternate forms are not currently selectable in the OCR helper.',
        'PokeMMO internal string IDs and ROM personal slots use different mappings.',
        'Names or a national number alone do not reliably identify every alternate form.',
    ]
    assert not missing_names and not result['union_missing_from_app']
    assert all(not row['type_mismatches'] for row in result['roms'])
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--rom-dir', type=Path, required=True)
    parser.add_argument('--strings', type=Path, required=True)
    parser.add_argument('--output', type=Path, default=ROOT / 'docs/evidence/species-coverage-audit.json')
    args = parser.parse_args()
    report = audit(args.rom_dir, args.strings)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(f"Checked {len(report['roms'])} ROMs: union {report['union_national_count']} national base species, no missing IDs or type mismatches; wrote {args.output}")
