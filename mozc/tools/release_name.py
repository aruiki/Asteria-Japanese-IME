"""GitHubの表示名を、版・段階・日本時間の日付の順にそろえる。タグは変えない。"""
import argparse
from datetime import date, datetime, timedelta, timezone
import re
import sys

JST = timezone(timedelta(hours=9))


def release_name(tag: str, published_on: date) -> str:
    match = re.fullmatch(r"v\d+\.\d+\.\d+(?:-(?:(beta|rc)\.([1-9]\d*)|pr([1-9]\d*)))?", tag)
    if not match:
        raise ValueError(f"対応していないリリースタグ: {tag}")
    channel, number, pull_request = match.groups()
    if pull_request:
        stage = "開発確認用"
    elif channel == "beta":
        stage = f"ベータ版{number}"
    elif channel == "rc":
        stage = f"リリース候補{number}"
    else:
        stage = "正式版"
    return f"Asteria {tag} — {stage}（{published_on.isoformat()}）"


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--date", type=date.fromisoformat,
                        default=datetime.now(JST).date(), help="日本時間の公開日 YYYY-MM-DD")
    args = parser.parse_args()
    try:
        print(release_name(args.tag, args.date))
    except ValueError as exc:
        parser.error(str(exc))
    return 0


if __name__ == "__main__":
    sys.exit(main())
