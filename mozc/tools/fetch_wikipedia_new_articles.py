"""zenz-v2.5の学習(2024年2月のWikipedia)より後に作られた日本語版の記事を、評価用に集める。

ページ作成ログ(logevents、名前空間0)を月ごとに一定数取り、リダイレクトと削除済みを除いて、
導入部の平文(TextExtracts)を保存する。取得条件・時刻・revidを残し、同じ条件で集め直せるようにする。
Wikimediaの利用方針に従いUser-Agentを付け、1要求ずつ間隔を空ける。本文はCC BY-SA 4.0。

使い方: python mozc/tools/fetch_wikipedia_new_articles.py --out DIR [--start 2024-03 --end 2026-09 --per-month 100]
"""
import argparse
import datetime
import json
from pathlib import Path
import sys
import time
import urllib.parse
import urllib.request

API = "https://ja.wikipedia.org/w/api.php"
UA = "AsteriaIME-eval-builder/0.1 (local evaluation set construction; contact via project maintainer)"


def get(params):
    url = API + "?" + urllib.parse.urlencode({**params, "format": "json", "formatversion": "2"})
    request = urllib.request.Request(url, headers={"User-Agent": UA})
    for attempt in range(5):
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                return json.load(response)
        except OSError:
            time.sleep(5 * (attempt + 1))
    raise RuntimeError("API request failed: " + url)


def months(start, end):
    y, m = map(int, start.split("-"))
    ey, em = map(int, end.split("-"))
    while (y, m) <= (ey, em):
        yield y, m
        y, m = (y + 1, 1) if m == 12 else (y, m + 1)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--start", default="2024-03")
    ap.add_argument("--end", default="2026-09")
    ap.add_argument("--per-month", type=int, default=100)
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    titles = []
    for y, m in months(args.start, args.end):
        # 月の中ほどから古い方へたどる(月初に偏らない)。
        start = datetime.datetime(y, m, 15, tzinfo=datetime.timezone.utc)
        params = dict(action="query", list="logevents", letype="create", lenamespace=0,
                      lestart=start.isoformat().replace("+00:00", "Z"), ledir="older",
                      lelimit=min(500, args.per_month * 3))
        picked = 0
        while picked < args.per_month:
            data = get(params)
            for event in data["query"]["logevents"]:
                if picked >= args.per_month:
                    break
                # 続きをたどって前の月に入ったら、その月の分は終わり。
                if not event["timestamp"].startswith(f"{y}-{m:02d}"):
                    picked = args.per_month
                    break
                if "title" in event and event.get("pageid"):
                    titles.append(dict(title=event["title"], created=event["timestamp"], month=f"{y}-{m:02d}"))
                    picked += 1
            time.sleep(1)
            if "continue" not in data:
                break
            params.update(data["continue"])
    print(f"titles {len(titles)}", file=sys.stderr, flush=True)
    rows = []
    for i in range(0, len(titles), 20):
        if i % 2000 == 0:
            print(f"extracts {i}/{len(titles)} rows {len(rows)}", file=sys.stderr, flush=True)
        chunk = titles[i:i + 20]
        data = get(dict(action="query", prop="extracts|info|pageprops|categories", exintro=1, explaintext=1,
                        cllimit="max", clshow="!hidden", redirects=0, titles="|".join(t["title"] for t in chunk)))
        pages = {p["title"]: p for p in data["query"].get("pages", [])}
        for t in chunk:
            p = pages.get(t["title"])
            if not p or p.get("missing") or p.get("redirect") or "disambiguation" in p.get("pageprops", {}):
                continue
            text = (p.get("extract") or "").strip()
            if text:
                categories = [c["title"].split(":", 1)[-1] for c in p.get("categories", [])]
                rows.append(dict(**t, pageid=p["pageid"], lastrevid=p.get("lastrevid"), categories=categories,
                                 extract=text))
        time.sleep(1)
    with open(args.out / "articles.jsonl", "w", encoding="utf-8") as f:
        for row in rows:
            f.write(json.dumps(row, ensure_ascii=False) + "\n")
    meta = dict(api=API, user_agent=UA, start=args.start, end=args.end, per_month=args.per_month,
                fetched_at=datetime.datetime.now(datetime.timezone.utc).isoformat(), titles=len(titles),
                articles=len(rows), license="CC BY-SA 4.0 (Wikipedia text)")
    (args.out / "meta.json").write_text(json.dumps(meta, ensure_ascii=False, indent=1), encoding="utf-8")
    print(json.dumps(meta, ensure_ascii=False))


if __name__ == "__main__":
    main()
