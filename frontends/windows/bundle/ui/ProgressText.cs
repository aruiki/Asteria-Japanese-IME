// 進み具合の文(MSI の処理の名前 → 日本語、扱っているファイルの名前、残り時間)。
using System;
using System.Text.RegularExpressions;

namespace Asteria.Setup
{
    public static class ProgressText
    {
        // MSI の ActionStart は「Action 12:00:00: InstallFiles. Copying new files」の形(OS の言語で前置きが変わる)。
        private static readonly Regex ActionName = new Regex(@":\s*([A-Za-z_][A-Za-z0-9_.]*)\.\s", RegexOptions.Compiled);
        // ActionData の中のファイル名(同梱物の拡張子だけを拾う。パスや値は出さない)。
        private static readonly Regex FileName = new Regex(
            @"([A-Za-z0-9_\-.]+\.(?:gguf|dll|exe|txt|json|tsv|html|png|ico))", RegexOptions.Compiled | RegexOptions.IgnoreCase);

        /// <summary>MSI の処理の名前から、画面に出す段階の文を決める。出さない処理は null。</summary>
        public static string ForAction(string message, bool uninstall)
        {
            if (string.IsNullOrEmpty(message)) return null;
            var m = ActionName.Match(message);
            if (!m.Success) return null;
            switch (m.Groups[1].Value)
            {
                case "InstallValidate":
                case "InstallInitialize":
                    return uninstall ? "外す準備をしています" : "入れる準備をしています";
                case "RemoveExistingProducts":
                    return "前の版を外しています";
                case "RemoveFiles":
                    return uninstall ? "ファイルを消しています" : "古いファイルを消しています";
                case "InstallFiles":
                    return "ファイルをコピーしています";
                case "WriteRegistryValues":
                case "RemoveRegistryValues":
                    return "設定を書き込んでいます";
                case "RegisterProduct":
                case "PublishFeatures":
                case "PublishProduct":
                case "UnpublishFeatures":
                    return "Windows に登録しています";
                case "InstallFinalize":
                    return "仕上げています";
                default:
                    return null;
            }
        }

        /// <summary>ActionData から扱っているファイルの名前を取り出す。なければ null。</summary>
        public static string FileIn(string message)
        {
            if (string.IsNullOrEmpty(message)) return null;
            var m = FileName.Match(message);
            return m.Success ? m.Groups[1].Value : null;
        }

        /// <summary>受け取った大きさの文(「0.84 GB / 1.57 GB」)。1 GB 未満は MB で出す。</summary>
        public static string Bytes(long done, long total)
        {
            const double GB = 1024.0 * 1024 * 1024, MB = 1024.0 * 1024;
            if (total >= GB) return $"{done / GB:0.00} GB / {total / GB:0.00} GB";
            return $"{done / MB:0} MB / {total / MB:0} MB";
        }

        /// <summary>経過時間と進み具合から残り時間の文を作る(3% までは見積もらない)。</summary>
        public static string Remaining(TimeSpan elapsed, int percent)
        {
            if (percent >= 100) return "";
            if (percent < 3 || elapsed.TotalSeconds < 3) return "残り時間を計算しています";
            double left = elapsed.TotalSeconds * (100 - percent) / percent;
            if (left < 60) return "残り 1 分未満";
            return $"残り約 {Math.Ceiling(left / 60)} 分";
        }
    }
}
