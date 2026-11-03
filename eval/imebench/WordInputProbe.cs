// Word の文書へ、実際のキー入力(SendInput)で日本語を打って確定し、文書に入った文字を確かめる(docs/adr/0081)。
//
// Word を COM で新しく開き(白紙の文書)、Asteria を有効にして、1 行ずつ「ローマ字 → 手を止める → Space → Enter(確定)
// → Enter(改行)」を送る。手を止めた間と Space の後に候補の窓(KotoriCandidateWindow)が見えたかも記録する。
// 終わったら文書の本文を COM で読み、保存せずに閉じる。文字の入力に COM は使わない。
//
// 使い方: WordInputProbe.exe <入力.tsv> <出力.json> <画像の保存先フォルダ>
//   入力.tsv: 1 行に「番号<TAB>ローマ字<TAB>期待する確定文字」。
// 利用者本人が実 PC での検証を許可した local_install_check.py からだけ実行する(KOTORI_LOCAL_ACCEPTANCE)。
// 計測の間、この PC のキーボード入力を使う(ほかの操作をしない)。
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Web.Script.Serialization;
using System.Windows.Forms;

[ComImport, Guid("71C6E74C-0F28-11D8-A82A-00065B84435C"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface ITfInputProcessorProfileMgr {
  [PreserveSig] int ActivateProfile(uint dwProfileType, ushort langid, ref Guid clsid, ref Guid guidProfile,
                                    IntPtr hkl, uint dwFlags);
}

static class WordNative {
  [StructLayout(LayoutKind.Sequential)]
  public struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Sequential)]
  public struct INPUT { public uint type; public KEYBDINPUT ki; public long pad; }
  public delegate bool WindowCallback(IntPtr window, IntPtr parameter);
  [DllImport("user32.dll", SetLastError = true)] public static extern uint SendInput(uint n, INPUT[] inputs, int size);
  [DllImport("user32.dll")] public static extern short VkKeyScanW(char ch);
  [DllImport("user32.dll")] public static extern uint MapVirtualKeyW(uint code, uint type);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int command);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint from, uint to, bool attach);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] public static extern bool EnumWindows(WindowCallback callback, IntPtr parameter);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder name, int size);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [StructLayout(LayoutKind.Sequential)]
  public struct RECT { public int Left, Top, Right, Bottom; }
  [StructLayout(LayoutKind.Sequential)]
  public struct GUITHREADINFO {
    public int cbSize; public uint flags; public IntPtr hwndActive, hwndFocus, hwndCapture, hwndMenuOwner, hwndMoveSize, hwndCaret;
    public RECT rcCaret;
  }
  [DllImport("user32.dll")] public static extern bool GetGUIThreadInfo(uint thread, ref GUITHREADINFO info);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder text, int size);

  // Word が文書の上に出しているダイアログ(NUIDialog)の題名。ライセンスの案内などで編集できない理由が分かる。
  public static List<string> DialogTitles(IntPtr owner) {
    uint pid;
    GetWindowThreadProcessId(owner, out pid);
    var titles = new List<string>();
    EnumWindows((window, unused) => {
      uint other;
      GetWindowThreadProcessId(window, out other);
      var name = new StringBuilder(128);
      GetClassNameW(window, name, name.Capacity);
      if (other == pid && name.ToString() == "NUIDialog" && IsWindowVisible(window)) {
        var title = new StringBuilder(256);
        GetWindowTextW(window, title, title.Capacity);
        titles.Add(title.ToString());
      }
      return true;
    }, IntPtr.Zero);
    return titles;
  }

  // 前面のアプリでキーボードを受け取っている窓のクラス名。
  public static string FocusClass() {
    var info = new GUITHREADINFO();
    info.cbSize = Marshal.SizeOf(typeof(GUITHREADINFO));
    if (!GetGUIThreadInfo(0, ref info) || info.hwndFocus == IntPtr.Zero) return "";
    var name = new StringBuilder(128);
    GetClassNameW(info.hwndFocus, name, name.Capacity);
    return name.ToString();
  }

  public static bool CandidateVisible() {
    bool found = false;
    EnumWindows((window, unused) => {
      var name = new StringBuilder(128);
      GetClassNameW(window, name, name.Capacity);
      if (name.ToString() == "KotoriCandidateWindow" && IsWindowVisible(window)) { found = true; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}

static class WordInputProbe {
  static IntPtr word;

  static void Require(bool value, string error) { if (!value) throw new Exception(error); }

  static void Key(ushort vk, bool up) {
    var ins = new WordNative.INPUT[1];
    ins[0].type = 1;
    ins[0].ki.wVk = vk;
    ins[0].ki.wScan = (ushort)WordNative.MapVirtualKeyW(vk, 0);
    ins[0].ki.dwFlags = up ? 2u : 0u;
    Require(WordNative.SendInput(1, ins, Marshal.SizeOf(typeof(WordNative.INPUT))) == 1, "SendInput failed");
  }

  static void Press(ushort vk) {
    Require(WordNative.GetForegroundWindow() == word, "Word lost foreground");
    Key(vk, false); Key(vk, true);
  }

  static void TypeKeys(string romaji) {
    foreach (char ch in romaji) {
      short r = WordNative.VkKeyScanW(ch);
      Require(r != -1, "Unmapped key: " + ch);
      bool shift = (r & 0x100) != 0;
      if (shift) Key(0x10, false);
      Press((ushort)(r & 0xff));
      if (shift) Key(0x10, true);
      Thread.Sleep(40);
    }
  }

  // ms の間、100 ms ごとに候補の窓を見て、一度でも見えたら true。
  static bool SeenWithin(int ms) {
    bool seen = false;
    for (int t = 0; t < ms; t += 100) {
      seen |= WordNative.CandidateVisible();
      Thread.Sleep(100);
    }
    return seen | WordNative.CandidateVisible();
  }

  // 前に出せないことがある(前面のロック)ので、前面のスレッドに入力をつないでから出す。
  static void Foreground() {
    WordNative.ShowWindow(word, 9);  // SW_RESTORE
    for (int i = 0; i < 20 && WordNative.GetForegroundWindow() != word; i++) {
      uint unused;
      uint front = WordNative.GetWindowThreadProcessId(WordNative.GetForegroundWindow(), out unused);
      uint self = WordNative.GetCurrentThreadId();
      WordNative.AttachThreadInput(self, front, true);
      WordNative.BringWindowToTop(word);
      WordNative.SetForegroundWindow(word);
      WordNative.AttachThreadInput(self, front, false);
      Thread.Sleep(250);
    }
    Require(WordNative.GetForegroundWindow() == word, "Word could not be brought to the foreground");
  }

  // 起動直後の Word は、文書の上に案内の枠(Web の表示)を出してキーボードを取ることがある。
  // キーボードが文書の本文(_WwG)に来るまで Esc で閉じる。
  static void FocusBody(dynamic doc, List<string> seen) {
    for (int i = 0; i < 10; i++) {
      Foreground();
      string focus = WordNative.FocusClass();
      seen.Add(focus);
      if (focus == "_WwG") return;
      Key(0x1B, false); Key(0x1B, true);
      Thread.Sleep(500);
      doc.Activate();
      Thread.Sleep(300);
    }
    // 例: 「5 日間のパスが終了しました」(Office のライセンス切れ。閉じても編集できない)。
    throw new Exception("Keyboard focus did not reach the document body: " + String.Join(",", seen) +
                        "; Word dialogs: " + String.Join(" | ", WordNative.DialogTitles(word)));
  }

  static void Shot(string folder, string name) {
    var area = Screen.PrimaryScreen.Bounds;
    using (var image = new Bitmap(area.Width, area.Height))
    using (var g = Graphics.FromImage(image)) {
      g.CopyFromScreen(area.Location, Point.Empty, area.Size);
      image.Save(Path.Combine(folder, name), ImageFormat.Png);
    }
  }

  [STAThread]
  static int Main(string[] args) {
    if (args.Length != 3 || Environment.GetEnvironmentVariable("KOTORI_LOCAL_ACCEPTANCE") != "owner-approved") return 2;
    var report = new Dictionary<string, object>();
    var rows = new List<Dictionary<string, object>>();
    report["trials"] = rows;
    dynamic app = null, doc = null;
    bool launched = false;
    Directory.CreateDirectory(args[2]);
    try {
      var cases = new List<string[]>();
      foreach (string line in File.ReadAllLines(args[0], Encoding.UTF8)) {
        string[] f = line.Split('\t');
        if (f.Length >= 3) cases.Add(f);
      }
      launched = Process.GetProcessesByName("WINWORD").Length == 0;
      app = Activator.CreateInstance(Type.GetTypeFromProgID("Word.Application"));
      app.Visible = true;
      doc = app.Documents.Add();
      doc.Activate();
      report["word_version"] = (string)app.Version;
      word = new IntPtr((int)app.ActiveWindow.Hwnd);
      Thread.Sleep(4000);
      Foreground();
      Shot(args[2], "word-start.png");
      var focusSeen = new List<string>();
      report["focus_seen"] = focusSeen;
      FocusBody(doc, focusSeen);

      Guid clsid = new Guid("2A1ADAE4-8061-4AC0-A7D4-713BF8DCB591");
      Guid profile = new Guid("455A87E7-275F-4582-92CB-6600917A436D");
      var manager = (ITfInputProcessorProfileMgr)Activator.CreateInstance(
          Type.GetTypeFromCLSID(new Guid("33C53A50-F456-4884-B049-85FD643ECFED")));
      Require(manager.ActivateProfile(1, 0x0411, ref clsid, ref profile, IntPtr.Zero, 0x10000005) == 0,
              "Session TIP activation failed");
      Thread.Sleep(800);
      Foreground();
      Press(0xF2);  // ひらがなを明示する。
      Thread.Sleep(500);

      foreach (string[] c in cases) {
        FocusBody(doc, focusSeen);
        TypeKeys(c[1]);
        bool paused = SeenWithin(2500);
        Press(0x20);  // Space(変換)
        bool afterSpace = SeenWithin(2000);
        Shot(args[2], "word-" + c[0] + ".png");
        Press(0x0D);  // 確定
        Thread.Sleep(400);
        Press(0x0D);  // 改行
        Thread.Sleep(400);
        rows.Add(new Dictionary<string, object> {
          {"id", c[0]}, {"romaji", c[1]}, {"expected", c[2]},
          {"window_on_pause", paused}, {"window_after_space", afterSpace}});
      }
      string body = ((string)doc.Content.Text).Replace("\r\a", "").TrimEnd('\r');
      report["body"] = body;
      string[] lines = body.Split('\r');
      int passed = 0;
      for (int i = 0; i < rows.Count; i++) {
        string got = i < lines.Length ? lines[i] : "";
        rows[i]["committed"] = got;
        rows[i]["passed"] = got == (string)rows[i]["expected"];
        if ((bool)rows[i]["passed"]) passed++;
      }
      report["line_count"] = lines.Length;
      report["passed"] = passed;
      report["complete"] = lines.Length == rows.Count;
    } catch (Exception e) {
      report["error"] = e.ToString();
      try { Shot(args[2], "word-error.png"); } catch (Exception) {}
    } finally {
      try { if (doc != null) doc.Close(0); } catch (Exception) {}       // wdDoNotSaveChanges
      try { if (app != null && launched) app.Quit(0); } catch (Exception) {}
    }
    File.WriteAllText(args[1], new JavaScriptSerializer().Serialize(report), new UTF8Encoding(false));
    return report.ContainsKey("error") ? 1 : 0;
  }
}
