#!/usr/bin/env python3
import argparse
import csv
import sqlite3
import sys
import tarfile
import urllib.request
from pathlib import Path

ECDICT_URL = "https://raw.githubusercontent.com/skywind3000/ECDICT/master/ecdict.csv"
WORDNET_URL = "https://wordnetcode.princeton.edu/3.0/WNdb-3.0.tar.gz"
WORDNET_DATA_FILES = ("data.noun", "data.verb", "data.adj", "data.adv")


def download(url: str, target: Path) -> Path:
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists():
        print(f"[cache] {target}")
        return target

    print(f"[download] {url}")
    with urllib.request.urlopen(url) as response, target.open("wb") as output:
        while True:
            chunk = response.read(1024 * 1024)
            if not chunk:
                break
            output.write(chunk)

    return target


def create_schema(connection: sqlite3.Connection) -> None:
    connection.executescript(
        """
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;

        CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS ecdict_entries (
            word TEXT PRIMARY KEY,
            phonetic TEXT,
            definition TEXT,
            translation TEXT,
            pos TEXT,
            exchange TEXT,
            tag TEXT
        );

        CREATE TABLE IF NOT EXISTS wordnet_synonyms (
            word TEXT NOT NULL,
            synonym TEXT NOT NULL,
            PRIMARY KEY (word, synonym)
        );

        CREATE TABLE IF NOT EXISTS wordnet_glosses (
            word TEXT NOT NULL,
            gloss TEXT NOT NULL,
            PRIMARY KEY (word, gloss)
        );

        CREATE TABLE IF NOT EXISTS wordnet_examples (
            word TEXT NOT NULL,
            example TEXT NOT NULL,
            PRIMARY KEY (word, example)
        );

        CREATE INDEX IF NOT EXISTS idx_wordnet_synonyms_word
        ON wordnet_synonyms(word);

        CREATE INDEX IF NOT EXISTS idx_wordnet_glosses_word
        ON wordnet_glosses(word);

        CREATE INDEX IF NOT EXISTS idx_wordnet_examples_word
        ON wordnet_examples(word);
        """
    )


def normalize_word(value: str) -> str:
    return value.strip().lower()


def import_ecdict(connection: sqlite3.Connection, csv_path: Path) -> None:
    print(f"[import] ECDICT <- {csv_path}")
    inserted = 0
    batch = []

    with csv_path.open("r", encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle)
        for row in reader:
            word = normalize_word(row["word"])
            if not word:
                continue

            batch.append(
                (
                    word,
                    row["phonetic"].strip() or None,
                    row["definition"].strip() or None,
                    row["translation"].strip() or None,
                    row["pos"].strip() or None,
                    row["exchange"].strip() or None,
                    row["tag"].strip() or None,
                )
            )

            if len(batch) >= 2000:
                connection.executemany(
                    """
                    INSERT OR REPLACE INTO ecdict_entries
                    (word, phonetic, definition, translation, pos, exchange, tag)
                    VALUES (?, ?, ?, ?, ?, ?, ?)
                    """,
                    batch,
                )
                inserted += len(batch)
                batch.clear()

    if batch:
        connection.executemany(
            """
            INSERT OR REPLACE INTO ecdict_entries
            (word, phonetic, definition, translation, pos, exchange, tag)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            """,
            batch,
        )
        inserted += len(batch)

    print(f"[done] ECDICT rows: {inserted}")


def parse_gloss(gloss: str) -> tuple[str | None, list[str]]:
    parts = [part.strip() for part in gloss.split(";") if part.strip()]
    if not parts:
        return None, []

    definition = parts[0].strip().strip('"') or None
    examples = []
    for part in parts[1:]:
        part = part.strip()
        if part.startswith('"') and part.endswith('"'):
            examples.append(part[1:-1].strip())

    return definition, examples


def iter_wordnet_members(archive: tarfile.TarFile):
    members = {Path(member.name).name: member for member in archive.getmembers()}
    for file_name in WORDNET_DATA_FILES:
        member = members.get(file_name)
        if member is None:
            raise RuntimeError(f"WordNet archive missing {file_name}")
        extracted = archive.extractfile(member)
        if extracted is None:
            raise RuntimeError(f"Unable to extract {file_name}")
        yield file_name, extracted


def import_wordnet(connection: sqlite3.Connection, archive_path: Path) -> None:
    print(f"[import] WordNet <- {archive_path}")
    synonym_batch = []
    gloss_batch = []
    example_batch = []
    processed = 0

    with tarfile.open(archive_path, "r:gz") as archive:
        for file_name, extracted in iter_wordnet_members(archive):
            print(f"[parse] {file_name}")
            for raw_line in extracted:
                line = raw_line.decode("utf-8", errors="ignore").strip()
                if not line or "|" not in line or line.startswith("Copyright"):
                    continue

                data_part, gloss = line.split("|", 1)
                parts = data_part.split()
                if len(parts) < 4:
                    continue

                try:
                    word_count = int(parts[3], 16)
                except ValueError:
                    continue

                words = []
                index = 4
                for _ in range(word_count):
                    if index + 1 >= len(parts):
                        break
                    words.append(normalize_word(parts[index].replace("_", " ")))
                    index += 2

                words = [word for word in dict.fromkeys(words) if word]
                if not words:
                    continue

                definition, examples = parse_gloss(gloss.strip())

                for word in words:
                    for synonym in words:
                        if synonym != word:
                            synonym_batch.append((word, synonym))
                    if definition:
                        gloss_batch.append((word, definition))
                    for example in examples:
                        if example:
                            example_batch.append((word, example))

                processed += 1
                if processed % 4000 == 0:
                    flush_wordnet_batches(connection, synonym_batch, gloss_batch, example_batch)

    flush_wordnet_batches(connection, synonym_batch, gloss_batch, example_batch)
    print(f"[done] WordNet synsets: {processed}")


def flush_wordnet_batches(
    connection: sqlite3.Connection,
    synonym_batch: list[tuple[str, str]],
    gloss_batch: list[tuple[str, str]],
    example_batch: list[tuple[str, str]],
) -> None:
    if synonym_batch:
        connection.executemany(
            "INSERT OR IGNORE INTO wordnet_synonyms (word, synonym) VALUES (?, ?)",
            synonym_batch,
        )
        synonym_batch.clear()

    if gloss_batch:
        connection.executemany(
            "INSERT OR IGNORE INTO wordnet_glosses (word, gloss) VALUES (?, ?)",
            gloss_batch,
        )
        gloss_batch.clear()

    if example_batch:
        connection.executemany(
            "INSERT OR IGNORE INTO wordnet_examples (word, example) VALUES (?, ?)",
            example_batch,
        )
        example_batch.clear()


def build_dictionary(output_path: Path, cache_dir: Path) -> None:
    ecdict_path = download(ECDICT_URL, cache_dir / "ecdict.csv")
    wordnet_path = download(WORDNET_URL, cache_dir / "WNdb-3.0.tar.gz")

    output_path.parent.mkdir(parents=True, exist_ok=True)
    if output_path.exists():
        output_path.unlink()

    connection = sqlite3.connect(output_path)
    try:
        create_schema(connection)
        import_ecdict(connection, ecdict_path)
        import_wordnet(connection, wordnet_path)
        connection.executemany(
            "INSERT OR REPLACE INTO meta (key, value) VALUES (?, ?)",
            [
                ("ecdict_url", ECDICT_URL),
                ("wordnet_url", WORDNET_URL),
            ],
        )
        connection.commit()
    finally:
        connection.close()

    print(f"[ok] built dictionary database: {output_path}")


def main() -> int:
    parser = argparse.ArgumentParser(description="Build bundled dictionary.db from ECDICT and WordNet")
    parser.add_argument(
        "--output",
        default=str(Path("src-tauri") / "resources" / "dictionary.db"),
        help="output sqlite file path",
    )
    parser.add_argument(
        "--cache-dir",
        default=str(Path(".cache") / "dictionary"),
        help="download cache directory",
    )
    args = parser.parse_args()

    build_dictionary(Path(args.output), Path(args.cache_dir))
    return 0


if __name__ == "__main__":
    sys.exit(main())
