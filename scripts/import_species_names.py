"""Build the bundled English/Japanese species-name table from PokeAPI's CSV.

Source: https://github.com/PokeAPI/pokeapi/blob/master/data/v2/csv/pokemon_species_names.csv
The CSV is factual name data only; no artwork, text, or code is copied. The
generated TSV is committed, so running the application needs no network.

Usage:
    python scripts/import_species_names.py --csv pokemon_species_names.csv \
        --out windows-ocr/data/species_names.tsv
"""
import argparse
import csv
import hashlib
from pathlib import Path

MAX_SPECIES = 649
LANGUAGE_EN = 9
LANGUAGE_JA_HRKT = 1
LANGUAGE_JA = 11


def load_names(csv_path):
    names = {}
    with csv_path.open(encoding="utf-8", newline="") as file:
        for row in csv.DictReader(file):
            try:
                species = int(row["pokemon_species_id"])
                language = int(row["local_language_id"])
            except (KeyError, TypeError, ValueError):
                raise ValueError("unexpected CSV columns")
            name = row["name"].strip()
            if not 1 <= species <= MAX_SPECIES or not name:
                continue
            names.setdefault(species, {})[language] = name
    return names


def pick(names, species, languages):
    for language in languages:
        name = names.get(species, {}).get(language)
        if name:
            return name
    return None


def validate(rows):
    for column, label in ((1, "English"), (2, "Japanese")):
        values = [row[column] for row in rows]
        duplicates = sorted({name for name in values if values.count(name) > 1})
        if duplicates:
            raise ValueError(f"duplicate {label} names: {duplicates[:10]}")
        for row in rows:
            name = row[column]
            if "\t" in name or "\n" in name or "\r" in name:
                raise ValueError(f"control character in {label} name for #{row[0]}")
            if not any(character.isalnum() for character in name):
                raise ValueError(f"empty-looking {label} name for #{row[0]}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--csv", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()

    names = load_names(args.csv)
    rows = []
    missing = []
    japanese_fallback = 0
    japanese_variants = 0
    for species in range(1, MAX_SPECIES + 1):
        english = pick(names, species, (LANGUAGE_EN,))
        japanese = pick(names, species, (LANGUAGE_JA_HRKT,))
        alternate = pick(names, species, (LANGUAGE_JA,))
        if japanese and alternate and japanese != alternate:
            japanese_variants += 1
        if not japanese:
            japanese = alternate
            japanese_fallback += 1
        if not english or not japanese:
            missing.append(species)
            continue
        rows.append((species, english, japanese))
    if missing:
        raise ValueError(f"missing names for species: {missing[:20]}")
    if len(rows) != MAX_SPECIES:
        raise ValueError(f"expected {MAX_SPECIES} species, got {len(rows)}")
    validate(rows)

    text = "# id\tEnglish name (PokeAPI en)\tJapanese name (PokeAPI ja-hrkt)\n"
    text += "".join(f"{species}\t{english}\t{japanese}\n" for species, english, japanese in rows)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(text, encoding="utf-8", newline="\n")
    digest = hashlib.sha256(args.out.read_bytes()).hexdigest()

    print(f"species={len(rows)}")
    print(f"japanese_from_ja={japanese_fallback} japanese_variants={japanese_variants}")
    print(f"sample={rows[0]} {rows[5]} {rows[24]} {rows[471]}")
    print(f"sha256={digest}")


if __name__ == "__main__":
    main()
