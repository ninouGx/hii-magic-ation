#!/usr/bin/env python3
"""Slice real Scryfall bulk files into small test fixtures.

Usage: scripts/build_fixtures.py <download_cache_dir>

Run by hand when the fixture card list changes. Tests never touch the network.
"""

import gzip
import json
import sys
import time
import urllib.request
from pathlib import Path

USER_AGENT = "hii-magic-ation-fixtures/0.1"
BULK_INDEX = "https://api.scryfall.com/bulk-data"
BULK_TYPES = ["oracle_cards", "default_cards", "rulings", "oracle_tags", "art_tags"]
FIXTURES = Path(__file__).resolve().parent.parent / "crates/ingest/tests/fixtures"
OUT_DIR = FIXTURES / "scryfall"
EDHREC_PAGES = "https://json.edhrec.com/pages/"
EDHREC_DIR = FIXTURES / "edhrec"

CARD_NAMES = [
    "Delver of Secrets // Insectile Aberration",
    "Valki, God of Lies // Tibalt, Cosmic Impostor",
    "Bonecrusher Giant // Stomp",
    "Fire // Ice",
    "Bushi Tenderfoot // Kenzo the Hardhearted",
    "Tarmogoyf",
    "Frodo, Adventurous Hobbit",
    "Sam, Loyal Attendant",
    "Wilson, Refined Grizzly",
    "Raised by Giants",
    "Teferi, Temporal Archmage",
    "Atraxa, Praetors' Voice",
    "Jace, Multiverse Architect",
    "Y'shtola, Night's Blessed",
    "Lightning Bolt",
    "Sol Ring",
    "Black Lotus",
    "Shivan Dragon",
    "Dragonlord Atarka",
    "Llanowar Elves",
]


def get(url):
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT, "Accept": "*/*"})
    with urllib.request.urlopen(request) as response:
        return response.read()


def download_bulks(cache):
    cache.mkdir(parents=True, exist_ok=True)
    index = json.loads(get(BULK_INDEX))["data"]
    paths = {}
    for entry in index:
        if entry["type"] not in BULK_TYPES:
            continue
        url = entry["jsonl_download_uri"]
        path = cache / url.rsplit("/", 1)[1]
        if not path.exists():
            print(f"downloading {url}")
            path.write_bytes(get(url))
            time.sleep(0.2)
        paths[entry["type"]] = path
    return paths


def read_jsonl(path):
    with gzip.open(path, "rt", encoding="utf-8") as lines:
        for line in lines:
            yield json.loads(line)


def write_jsonl(name, rows):
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    with gzip.open(OUT_DIR / f"{name}.jsonl.gz", "wt", encoding="utf-8") as out:
        for row in rows:
            out.write(json.dumps(row, ensure_ascii=False) + "\n")
    print(f"{name}: {len(rows)} rows")


def slice_tags(path, target_key, targets):
    tags = {tag["id"]: tag for tag in read_jsonl(path)}
    kept = {}
    for tag in tags.values():
        taggings = [t for t in tag["taggings"] if t.get(target_key) in targets]
        if taggings:
            kept[tag["id"]] = {**tag, "taggings": taggings}
    pending = [p for tag in kept.values() for p in tag["parent_ids"]]
    while pending:
        parent_id = pending.pop()
        if parent_id in kept or parent_id not in tags:
            continue
        kept[parent_id] = {**tags[parent_id], "taggings": []}
        pending.extend(tags[parent_id]["parent_ids"])
    return list(kept.values())


def edhrec_page(path):
    time.sleep(1)
    return json.loads(get(EDHREC_PAGES + path))


def keep_views(views, names, unknown):
    kept = [v for v in views if v["name"] in names or set(v.get("names", [])) <= names and v.get("names")]
    kept.append({**views[-1], "id": "00000000-0000-0000-0000-000000000000", "name": unknown, "sanitized": "unknown"})
    return kept


def write_edhrec(path, page):
    out = EDHREC_DIR / path
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(page, ensure_ascii=False, indent=1))
    print(f"edhrec {path}")


def build_edhrec_fixtures(names):
    """Page 1 nests the list under container.json_dict.cardlists[0], page 2 is flat."""
    first = edhrec_page("commanders/week.json")
    second_path = first["container"]["json_dict"]["cardlists"][0]["more"]
    second = edhrec_page(second_path)

    first_list = first["container"]["json_dict"]["cardlists"][0]
    first_list["cardviews"] = keep_views(first_list["cardviews"], names, "Unknown Commander One")
    write_edhrec("commanders/week.json", first)

    second["cardviews"] = keep_views(second["cardviews"], names, "Unknown Commander Two")
    second["more"] = None
    write_edhrec(second_path, second)

    for window in ("month", "year"):
        page = json.loads(json.dumps(first))
        page["container"]["json_dict"]["cardlists"][0]["more"] = None
        write_edhrec(f"commanders/{window}.json", page)


def main():
    paths = download_bulks(Path(sys.argv[1]))

    wanted = set(CARD_NAMES)
    cards = [c for c in read_jsonl(paths["oracle_cards"]) if c["name"] in wanted]
    missing = wanted - {c["name"] for c in cards}
    if missing:
        sys.exit(f"cards not found: {sorted(missing)}")
    oracle_ids = {c["oracle_id"] for c in cards}

    prints = [p for p in read_jsonl(paths["default_cards"]) if p.get("oracle_id") in oracle_ids]
    illustration_ids = {p["illustration_id"] for p in prints if p.get("illustration_id")}
    rulings = [r for r in read_jsonl(paths["rulings"]) if r["oracle_id"] in oracle_ids]

    write_jsonl("oracle_cards", cards)
    write_jsonl("default_cards", prints)
    write_jsonl("rulings", rulings)
    write_jsonl("oracle_tags", slice_tags(paths["oracle_tags"], "oracle_id", oracle_ids))
    write_jsonl("art_tags", slice_tags(paths["art_tags"], "illustration_id", illustration_ids))
    build_edhrec_fixtures(wanted)


if __name__ == "__main__":
    main()
