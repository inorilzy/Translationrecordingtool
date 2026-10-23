#!/usr/bin/env python3
import argparse
import csv
import os
import tempfile
import sqlite3
import sys
import tarfile
import urllib.request
from pathlib import Path

from asset_manifest import find_asset, load_manifest, verify_file

ECDICT_REVISION = "bc015ed2e24a7abef49fc6dbbb7fe32c1dadaf8b"
WORDNET_VERSION = "3.0"
ECDICT_URL = f"https://raw.githubusercontent.com/skywind3000/ECDICT/{ECDICT_REVISION}/ecdict.csv"
WORDNET_URL = f"https://wordnetcode.princeton.edu/{WORDNET_VERSION}/WNdb-{WORDNET_VERSION}.tar.gz"

WORDNET_DATA_FILES = ("data.noun", "data.verb", "data.adj", "data.adv")


def fetch_url(url: str, target: Path) -> None:
    """Transport only; the caller verifies the temporary file before publication."""
    print(f"[download] {url}")
    with urllib.request.urlopen(url) as response, target.open("wb") as output:
        while True:
            chunk = response.read(1024 * 1024)
            if not chunk:
                break
            output.write(chunk)


def validate_ecdict(path: Path, _entry: dict) -> None:
    """Structural validation only: no trusted upstream digest is available."""
    with path.open("r", encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle, strict=True)
        if not reader.fieldnames or "word" not in reader.fieldnames:
            raise RuntimeError("ECDICT CSV missing word column")
        valid_rows = 0
        for row in reader:
            if None in row or any(value is None for value in row.values()):
                raise RuntimeError("ECDICT CSV malformed row")
            if row["word"].strip():
                valid_rows += 1
        if not valid_rows:
            raise RuntimeError("ECDICT CSV has no valid words")


def validate_wordnet(path: Path, _entry: dict) -> None:
    """Check required members and read their bytes without extracting to disk."""
    with tarfile.open(path, "r:gz") as archive:
        for name, extracted in iter_wordnet_members(archive):
            with extracted:
                size = 0
                while chunk := extracted.read(1024 * 1024):
                    size += len(chunk)
                if not size:
                    raise RuntimeError(f"WordNet archive has empty {name}")


def download(file_entry: dict, target: Path, *, downloader=None, validator=verify_file) -> Path:
    """Reject corrupt caches; never overwrite a cache with unverified bytes."""
    if target.exists():
        validator(target, file_entry)
        print(f"[cache] {target}")
        return target

    target.parent.mkdir(parents=True, exist_ok=True)
    # Same filesystem as target, unique per attempt, removed on every failure.
    with tempfile.TemporaryDirectory(prefix=".dictionary-download-", dir=target.parent) as directory:
        temporary = Path(directory) / file_entry["name"]
        (downloader or fetch_url)(file_entry["url"], temporary)
        validator(temporary, file_entry)
        os.replace(temporary, target)
    return target


def create_schema(connection: sqlite3.Connection) -> None:
    connection.executescript(
        """
        PRAGMA journal_mode = DELETE;
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
                    row.get("phonetic", "").strip() or None,
                    row.get("definition", "").strip() or None,
                    row.get("translation", "").strip() or None,
                    row.get("pos", "").strip() or None,
                    row.get("exchange", "").strip() or None,
                    row.get("tag", "").strip() or None,
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
        if member is None or not member.isfile():
            raise RuntimeError(f"WordNet archive missing regular file {file_name}")
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


def validate_database(connection: sqlite3.Connection) -> None:
    if connection.execute("PRAGMA quick_check").fetchall() != [("ok",)]:
        raise RuntimeError("Dictionary SQLite quick_check failed")
    for table in ("ecdict_entries", "wordnet_glosses"):
        if not connection.execute(f"SELECT 1 FROM {table} LIMIT 1").fetchone():
            raise RuntimeError(f"Dictionary table {table} is empty")


def build_dictionary(
    output_path: Path, cache_dir: Path, *,
    manifest_path: Path | None = None, downloader=None,
) -> None:
    if manifest_path is not None:
        # Optional strict mode resolves both identities before any writes.
        # No synthetic opt-in exists in the production entry point.
        manifest = load_manifest(manifest_path)
        entries = [find_asset(manifest, "dictionary-input", {"source": source})["files"][0]
                   for source in ("ecdict", "wordnet")]
        paths = [cache_dir / entry["name"] for entry in entries]
        validators = [verify_file, verify_file]
    else:
        # Fixed official inputs, structurally checked without inventing digests.
        entries = [{"name": "ecdict.csv", "url": ECDICT_URL},
                   {"name": f"WNdb-{WORDNET_VERSION}.tar.gz", "url": WORDNET_URL}]
        paths = [cache_dir / "ecdict" / ECDICT_REVISION / entries[0]["name"],
                 cache_dir / "wordnet" / WORDNET_VERSION / entries[1]["name"]]
        validators = [validate_ecdict, validate_wordnet]
    if any(output_path.resolve() == path.resolve() for path in paths):
        raise ValueError("Dictionary output must not overwrite an input cache")
    ecdict_path, wordnet_path = [
        download(entry, path, downloader=downloader, validator=validator)
        for entry, path, validator in zip(entries, paths, validators)
    ]

    output_path.parent.mkdir(parents=True, exist_ok=True)
    # Keep the old database untouched until a fully committed, checked standalone
    # database is closed. DELETE journal avoids publishing a DB dependent on a WAL.
    with tempfile.TemporaryDirectory(prefix=".dictionary-build-", dir=output_path.parent) as directory:
        temporary = Path(directory) / "dictionary.db"
        connection = sqlite3.connect(temporary)
        try:
            create_schema(connection)
            import_ecdict(connection, ecdict_path)
            import_wordnet(connection, wordnet_path)
            connection.executemany(
                "INSERT OR REPLACE INTO meta (key, value) VALUES (?, ?)",
                [("ecdict_url", entries[0]["url"]), ("wordnet_url", entries[1]["url"])],
            )
            connection.commit()
            validate_database(connection)
        finally:
            connection.close()
        # Never pair a new main file with an old live/hot SQLite journal.
        if any(Path(str(output_path) + suffix).exists() for suffix in ("-wal", "-shm", "-journal")):
            raise RuntimeError("Dictionary output has SQLite sidecars; close its users before rebuilding")
        os.replace(temporary, output_path)

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
    parser.add_argument(
        "--manifest", default=None,
        help="optional strict release asset manifest (synthetic manifests are rejected)",
    )
    args = parser.parse_args()

    build_dictionary(Path(args.output), Path(args.cache_dir), manifest_path=Path(args.manifest) if args.manifest is not None else None)
    return 0


if __name__ == "__main__":
    sys.exit(main())
