"""Offline tests only: hashes derive from small synthetic bytes, never real assets."""
import copy
from contextlib import closing
import hashlib
import io
import json
from pathlib import Path
import sqlite3
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch, Mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import asset_manifest as am
import build_dictionary as bd

FIXTURES = Path(__file__).parent / "fixtures"


class DictionaryIntegrityTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="dictionary-integrity-test-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.cache = self.root / "cache"
        self.output = self.root / "dictionary.db"
        self.manifest_path = self.root / "synthetic-manifest.json"
        self.csv = (FIXTURES / "dictionary-synthetic.csv").read_bytes()
        self.archive = self.make_archive()
        base = json.loads((FIXTURES / "asset-manifest" / "synthetic-valid.json").read_text(encoding="utf-8"))
        self.manifest = {"schemaVersion": 1, "synthetic": True,
                         "assets": [a for a in base["assets"] if a["kind"] == "dictionary-input"]}
        self.payloads = {}
        for asset, payload in zip(self.manifest["assets"], (self.csv, self.archive)):
            entry = asset["files"][0]
            entry.update(size=len(payload), sha256=hashlib.sha256(payload).hexdigest())
            self.payloads[entry["url"]] = payload
        self.save_manifest()
        # Fail any accidental network access; all successful transports are injected.
        self.network = self.enterContext(patch.object(bd.urllib.request, "urlopen", side_effect=AssertionError("network forbidden")))
        self.downloader = Mock(side_effect=lambda url, target: target.write_bytes(self.payloads[url]))

    def make_archive(self, missing=False, empty=False):
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
            for name in bd.WORDNET_DATA_FILES:
                if missing and name == "data.adv":
                    continue
                data = b"Copyright synthetic\n" if empty else (FIXTURES / "dictionary-synthetic-wordnet.txt").read_bytes()
                member = tarfile.TarInfo("dict/" + name)
                member.size = len(data)
                archive.addfile(member, io.BytesIO(data))
        return buffer.getvalue()

    def save_manifest(self):
        self.manifest_path.write_text(json.dumps(self.manifest), encoding="utf-8")

    def build(self):
        # Test-only adapter; real build still invokes the strict loader, never an
        # allow-synthetic flag exposed through CLI or production build arguments.
        with patch.object(bd, "load_manifest", side_effect=lambda path: am.load_manifest(path, allow_synthetic=True)):
            bd.build_dictionary(self.output, self.cache, manifest_path=self.manifest_path, downloader=self.downloader)

    def old_database(self):
        connection = sqlite3.connect(self.output)
        connection.execute("CREATE TABLE sentinel(value TEXT)")
        connection.execute("INSERT INTO sentinel VALUES ('old valid database')")
        connection.commit()
        connection.close()
        return self.output.read_bytes()

    def assert_no_temps(self):
        self.assertEqual(list(self.root.rglob(".dictionary-*")), [])

    def default_build(self):
        bd.build_dictionary(self.output, self.cache, downloader=self.downloader)

    def default_payloads(self):
        self.payloads = {bd.ECDICT_URL: self.csv, bd.WORDNET_URL: self.archive}

    def test_default_without_manifest_fixed_urls_and_isolated_cache(self):
        self.default_payloads()
        self.manifest_path.unlink()
        self.cache.mkdir()
        # Old mutable-master cache and other revisions must never be reused.
        legacy = self.cache / "ecdict.csv"
        legacy.write_bytes(b"old mutable master")
        other = self.cache / "ecdict" / "other-revision" / "ecdict.csv"
        other.parent.mkdir(parents=True)
        other.write_bytes(b"other revision")
        (self.cache / "WNdb-3.0.tar.gz").write_bytes(b"legacy archive")
        self.old_database()
        with patch.object(bd, "load_manifest", side_effect=AssertionError("no manifest needed")):
            self.default_build()
        self.assertEqual([call.args[0] for call in self.downloader.call_args_list], [
            "https://raw.githubusercontent.com/skywind3000/ECDICT/bc015ed2e24a7abef49fc6dbbb7fe32c1dadaf8b/ecdict.csv",
            "https://wordnetcode.princeton.edu/3.0/WNdb-3.0.tar.gz",
        ])
        self.assertEqual((self.cache / "ecdict" / bd.ECDICT_REVISION / "ecdict.csv").read_bytes(), self.csv)
        self.assertEqual((self.cache / "wordnet" / "3.0" / "WNdb-3.0.tar.gz").read_bytes(), self.archive)
        self.assertEqual(legacy.read_bytes(), b"old mutable master")
        self.assertEqual(other.read_bytes(), b"other revision")
        with closing(sqlite3.connect(self.output)) as connection:
            bd.validate_database(connection)
            self.assertEqual(connection.execute("SELECT COUNT(*) FROM ecdict_entries").fetchone(), (2,))
            self.assertEqual(dict(connection.execute("SELECT * FROM meta")),
                             {"ecdict_url": bd.ECDICT_URL, "wordnet_url": bd.WORDNET_URL})
        self.downloader.reset_mock()
        self.downloader.side_effect = AssertionError("cache must be offline")
        self.default_build()
        self.downloader.assert_not_called()
        self.network.assert_not_called()
        self.assert_no_temps()

    def test_default_cli_uses_production_downloader_seam(self):
        self.default_payloads()
        with patch.object(sys, "argv", ["build_dictionary.py", "--output", str(self.output),
                                         "--cache-dir", str(self.cache)]), \
                patch.object(bd, "fetch_url", self.downloader):
            self.assertEqual(bd.main(), 0)
        self.assertEqual(self.downloader.call_count, 2)
        self.network.assert_not_called()

    def test_default_bad_csv_rejected_before_publication(self):
        old = self.old_database()
        self.default_payloads()
        for payload in (b"", b"other\napple\n", b"word\n", b"word\n   \n",
                        b"word,definition\napple\n", b'word\n"unterminated', b"word\n\xff\n"):
            with self.subTest(payload=payload):
                self.payloads[bd.ECDICT_URL] = payload
                with self.assertRaises((RuntimeError, UnicodeError, bd.csv.Error)):
                    self.default_build()
                self.assertFalse((self.cache / "ecdict" / bd.ECDICT_REVISION / "ecdict.csv").exists())
                self.assertEqual(self.output.read_bytes(), old)
                self.assert_no_temps()

    def test_default_bad_archive_rejected_before_publication(self):
        old = self.old_database()
        self.default_payloads()
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
            for name in bd.WORDNET_DATA_FILES:
                archive.addfile(tarfile.TarInfo("dict/" + name), io.BytesIO())
        for payload in (b"", b"not gzip", self.make_archive(missing=True), buffer.getvalue(), self.archive[:30]):
            with self.subTest(size=len(payload)):
                self.payloads[bd.WORDNET_URL] = payload
                with self.assertRaises((RuntimeError, tarfile.TarError, EOFError)):
                    self.default_build()
                self.assertFalse((self.cache / "wordnet" / "3.0" / "WNdb-3.0.tar.gz").exists())
                self.assertEqual(self.output.read_bytes(), old)
                self.assert_no_temps()

    def test_default_empty_wordnet_import_preserves_database(self):
        self.default_payloads()
        self.payloads[bd.WORDNET_URL] = self.make_archive(empty=True)
        old = self.old_database()
        with self.assertRaisesRegex(RuntimeError, "wordnet_glosses is empty"):
            self.default_build()
        self.assertEqual(self.output.read_bytes(), old)
        self.assert_no_temps()

    def test_default_interrupted_download_preserves_database(self):
        old = self.old_database()
        def failing(url, target):
            target.write_bytes(b"partial")
            raise OSError("download interruption")
        self.downloader.side_effect = failing
        with self.assertRaisesRegex(OSError, "interruption"):
            self.default_build()
        self.assertFalse((self.cache / "ecdict" / bd.ECDICT_REVISION / "ecdict.csv").exists())
        self.assertEqual(self.output.read_bytes(), old)
        self.assert_no_temps()

    def test_default_output_cannot_overwrite_versioned_input(self):
        self.output = self.cache / "ecdict" / bd.ECDICT_REVISION / "ecdict.csv"
        with self.assertRaisesRegex(ValueError, "input cache"):
            self.default_build()
        self.downloader.assert_not_called()
        self.assertFalse(self.cache.exists())

    def test_default_cache_revalidated_and_database_failure_protected(self):
        self.default_payloads()
        self.default_build()
        old = self.output.read_bytes()
        for path, payload in ((self.cache / "ecdict" / bd.ECDICT_REVISION / "ecdict.csv", self.csv),
                              (self.cache / "wordnet" / "3.0" / "WNdb-3.0.tar.gz", self.archive)):
            path.write_bytes(b"corrupt")
            self.downloader.reset_mock()
            with self.assertRaises((RuntimeError, tarfile.TarError)):
                self.default_build()
            self.downloader.assert_not_called()
            self.assertEqual(path.read_bytes(), b"corrupt")
            self.assertEqual(self.output.read_bytes(), old)
            path.write_bytes(payload)
        for function in ("import_ecdict", "import_wordnet", "validate_database"):
            with patch.object(bd, function, side_effect=RuntimeError("build failure")):
                with self.assertRaisesRegex(RuntimeError, "build failure"):
                    self.default_build()
            self.assertEqual(self.output.read_bytes(), old)
        self.assert_no_temps()

    def test_download_build_and_verified_offline_cache(self):
        self.old_database()
        self.build()
        self.assertEqual(self.downloader.call_count, 2)
        connection = sqlite3.connect(self.output)
        try:
            self.assertEqual(connection.execute("SELECT word FROM ecdict_entries ORDER BY word").fetchall(), [("apple",), ("pear",)])
            self.assertIn(("apple", "synthetic fruit"), connection.execute("SELECT * FROM wordnet_synonyms").fetchall())
            self.assertIn(("apple", "a synthetic example"), connection.execute("SELECT * FROM wordnet_examples").fetchall())
            self.assertEqual(connection.execute("PRAGMA quick_check").fetchall(), [("ok",)])
            self.assertEqual(dict(connection.execute("SELECT * FROM meta"))["ecdict_url"], self.manifest["assets"][0]["files"][0]["url"])
        finally:
            connection.close()
        self.downloader.reset_mock()
        self.downloader.side_effect = AssertionError("cache must be offline")
        self.build()
        self.downloader.assert_not_called()
        self.assert_no_temps()

    def test_cache_revalidated_each_time(self):
        self.build()
        old = self.output.read_bytes()
        for asset in self.manifest["assets"]:
            entry = asset["files"][0]
            original = self.payloads[entry["url"]]
            for corrupt in (original[:-1], b"!" + original[1:]):
                with self.subTest(name=entry["name"], size=len(corrupt)):
                    (self.cache / entry["name"]).write_bytes(corrupt)
                    self.downloader.reset_mock()
                    with patch.object(bd, "import_ecdict") as importer:
                        with self.assertRaises(am.AssetManifestError):
                            self.build()
                        importer.assert_not_called()
                    self.downloader.assert_not_called()
                    self.assertEqual(self.output.read_bytes(), old)
                    self.assertEqual((self.cache / entry["name"]).read_bytes(), corrupt)
            (self.cache / entry["name"]).write_bytes(original)
        self.assert_no_temps()

    def test_bad_download_hash_and_truncation_never_import(self):
        old = self.old_database()
        for payload in (self.csv[:-1], b"!" + self.csv[1:]):
            with self.subTest(size=len(payload)):
                self.downloader.side_effect = lambda url, target: target.write_bytes(payload)
                with patch.object(bd, "import_ecdict") as importer:
                    with self.assertRaises(am.AssetManifestError):
                        self.build()
                    importer.assert_not_called()
                self.assertFalse((self.cache / "ecdict.csv").exists())
                self.assertEqual(self.output.read_bytes(), old)
                self.assert_no_temps()

    def test_download_failure_preserves_existing_input_and_database(self):
        old = self.old_database()
        self.cache.mkdir()
        (self.cache / "ecdict.csv").write_bytes(self.csv)
        def failing(url, target):
            target.write_bytes(b"partial archive")
            raise OSError("synthetic download interruption")
        self.downloader.side_effect = failing
        with self.assertRaisesRegex(OSError, "interruption"):
            self.build()
        self.assertEqual((self.cache / "ecdict.csv").read_bytes(), self.csv)
        self.assertFalse((self.cache / "WNdb-3.0.tar.gz").exists())
        self.assertEqual(self.output.read_bytes(), old)
        self.assert_no_temps()

    def test_missing_download_file_rejected(self):
        self.downloader.side_effect = lambda url, target: None
        with self.assertRaisesRegex(am.AssetManifestError, "file not found"):
            self.build()
        self.assertFalse(self.output.exists())
        self.assert_no_temps()

    def test_build_failures_preserve_old_database(self):
        old = self.old_database()
        for function in ("import_ecdict", "import_wordnet", "validate_database"):
            with self.subTest(function=function):
                with patch.object(bd, function, side_effect=RuntimeError("synthetic build failure")):
                    with self.assertRaisesRegex(RuntimeError, "build failure"):
                        self.build()
                self.assertEqual(self.output.read_bytes(), old)
                self.assert_no_temps()

    def test_invalid_archive_and_empty_import_preserve_old_database(self):
        old = self.old_database()
        for payload in (b"not a gzip archive", self.make_archive(missing=True), self.make_archive(empty=True)):
            with self.subTest(size=len(payload)):
                entry = self.manifest["assets"][1]["files"][0]
                entry.update(size=len(payload), sha256=hashlib.sha256(payload).hexdigest())
                self.payloads[entry["url"]] = payload
                self.save_manifest()
                # Isolated per-attempt cache, not deletion of existing caches.
                self.cache = self.root / ("cache-" + str(len(payload)))
                with self.assertRaises((tarfile.TarError, RuntimeError)):
                    self.build()
                self.assertEqual(self.output.read_bytes(), old)
                self.assert_no_temps()

    def test_manifest_missing_unapproved_identity_and_entry_rejected_before_io(self):
        old = self.old_database()
        original = copy.deepcopy(self.manifest)
        variants = []
        bad = copy.deepcopy(original)
        bad["assets"][0]["identity"]["repo"] = "unapproved/repo"
        variants.append(bad)
        bad = copy.deepcopy(original)
        bad["assets"][0]["files"][0]["url"] = "https://example.invalid/ecdict.csv"
        variants.append(bad)
        bad = copy.deepcopy(original)
        bad["assets"][0]["approval"]["status"] = "pending"
        variants.append(bad)
        bad = copy.deepcopy(original)
        bad["assets"].pop()
        variants.append(bad)
        for data in variants:
            self.manifest = data
            self.save_manifest()
            with self.assertRaises(am.AssetManifestError):
                self.build()
        self.manifest_path.unlink()
        with self.assertRaisesRegex(am.AssetManifestError, "manifest file not found"):
            self.build()
        self.downloader.assert_not_called()
        self.assertFalse(self.cache.exists())
        self.assertEqual(self.output.read_bytes(), old)

    def test_production_entry_rejects_synthetic(self):
        with self.assertRaisesRegex(am.AssetManifestError, "synthetic"):
            bd.build_dictionary(self.output, self.cache, manifest_path=self.manifest_path, downloader=self.downloader)
        self.downloader.assert_not_called()
        self.assertFalse(self.cache.exists())

    def test_replace_failure_preserves_old_database(self):
        self.build()  # populate validated input caches before mocking replacement
        old = self.output.read_bytes()
        with patch.object(bd.os, "replace", side_effect=PermissionError("synthetic locked target")):
            with self.assertRaises(PermissionError):
                self.build()
        self.assertEqual(self.output.read_bytes(), old)
        self.assert_no_temps()

    def test_output_sidecars_are_not_deleted_or_mixed(self):
        old = self.old_database()
        sidecar = Path(str(self.output) + "-wal")
        sidecar.write_bytes(b"synthetic sidecar")
        with self.assertRaisesRegex(RuntimeError, "sidecars"):
            self.build()
        self.assertEqual(self.output.read_bytes(), old)
        self.assertEqual(sidecar.read_bytes(), b"synthetic sidecar")
        self.assert_no_temps()

    def test_output_cannot_overwrite_input(self):
        self.output = self.cache / "ecdict.csv"
        with self.assertRaisesRegex(ValueError, "input cache"):
            self.build()
        self.downloader.assert_not_called()
        self.assertFalse(self.cache.exists())


if __name__ == "__main__":
    unittest.main()
