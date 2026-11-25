// インストールした利用者の「入力方式の一覧」(Windows + Space) に Asteria を登録・整理する。
using System;
using System.Runtime.InteropServices;
using Microsoft.Win32;

namespace Asteria.Setup
{
    internal static class InputList
    {
        private const string CurrentTip = "0411:{797AD563-3368-4DCB-B3E7-8B0306B42B67}{D8E58E42-4B5C-4E34-88D5-355F38FE91A4}";
        private const string LegacyClsid = "{2A1ADAE4-8061-4AC0-A7D4-713BF8DCB591}";
        private const string LegacyTip = "0411:{2A1ADAE4-8061-4AC0-A7D4-713BF8DCB591}{455A87E7-275F-4582-92CB-6600917A436D}";

        private const uint ILOT_UNINSTALL = 0x00000001;

        [DllImport("input.dll", CharSet = CharSet.Unicode)]
        private static extern bool InstallLayoutOrTip(string list, uint flags);

        /// <summary>現在のユーザーの入力方式へ Asteria を追加し、孤立した旧 Mozc 版があれば外す。</summary>
        public static string Ensure()
        {
            string result = "";
            try
            {
                if (!LegacyDllExists())
                {
                    result += "legacy-removed=" + InstallLayoutOrTip(LegacyTip, ILOT_UNINSTALL) + " ";
                }
                result += "current-added=" + InstallLayoutOrTip(CurrentTip, 0);
            }
            catch (Exception e)
            {
                result += "error " + e.GetType().Name;
            }
            return result;
        }

        /// <summary>アンインストール時に現在のユーザーの入力方式一覧から Asteria を外す。</summary>
        public static string Remove()
        {
            try
            {
                return "current-removed=" + InstallLayoutOrTip(CurrentTip, ILOT_UNINSTALL);
            }
            catch (Exception e)
            {
                return "error " + e.GetType().Name;
            }
        }

        private static bool LegacyDllExists()
        {
            foreach (var view in new[] { RegistryView.Registry64, RegistryView.Registry32 })
            {
                using (var hklm = RegistryKey.OpenBaseKey(RegistryHive.LocalMachine, view))
                using (var key = hklm.OpenSubKey(@"SOFTWARE\Classes\CLSID\" + LegacyClsid + @"\InprocServer32"))
                {
                    var path = key?.GetValue("") as string;
                    if (!string.IsNullOrEmpty(path) && System.IO.File.Exists(path)) return true;
                }
            }
            return false;
        }
    }
}
