// 固定した公開文字列を使い、実TSFの確定回数とホストの挿入結果を照合する。
// 利用者のデスクトップでは実行しない。installer_smoke.pyの使い捨てCIガードを通す。
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

static class DuplicateNative {
  delegate bool WindowCallback(IntPtr window, IntPtr parameter);
  [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
  [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; }
  [DllImport("user32.dll", CharSet = CharSet.Unicode)]
  public static extern IntPtr FindWindowW(string className, string title);
  [DllImport("user32.dll")] static extern bool EnumWindows(WindowCallback callback, IntPtr parameter);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)]
  static extern int GetClassNameW(IntPtr window, StringBuilder text, int maximum);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out Rect rect);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(Point point);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint x, uint y, uint data, UIntPtr extra);
  public static IntPtr VisibleCandidate() {
    IntPtr found = IntPtr.Zero;
    EnumWindows((window, unused) => {
      var name = new StringBuilder(128); GetClassNameW(window, name, name.Capacity);
      if (name.ToString() == "KotoriCandidateWindow" && IsWindowVisible(window)) {
        found = window; return false;
      }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}

sealed partial class Acceptance {
  void FastType(ImeObservation edit) {
    Ui(() => Require(edit.Edit.ContainsFocus && AcceptanceNative.GetForegroundWindow() == Handle,
                     "Duplicate probe lost its own input target"));
    foreach (char key in "go-runisetteishite") {
      short mapped = Native.VkKeyScanW(key);
      Require(mapped != -1 && (mapped & 0xff00) == 0, "Unexpected fixed-key mapping");
      Key((ushort)mapped, false); Key((ushort)mapped, true); Thread.Sleep(5);
    }
  }
  string[] Results(ImeObservation edit) {
    var values = new List<string>();
    Ui(() => {
      foreach (var item in edit.Events) {
        object value;
        if (item.TryGetValue("result", out value) && ((string)value).Length > 0) values.Add((string)value);
      }
    });
    return values.ToArray();
  }
  void RawPress(ushort key) { Key(key, false); Key(key, true); }
  void ClickFirstCandidate(Dictionary<string, object> row) {
    // 最初のSpaceは変換表示だけの場合がある。一覧を開いてから実クリックする。
    if (DuplicateNative.VisibleCandidate() == IntPtr.Zero) RawPress(0x28);
    IntPtr candidate = IntPtr.Zero;
    WaitUntil(() => {
      candidate = DuplicateNative.VisibleCandidate();
      return candidate != IntPtr.Zero;
    }, "No visible installed candidate window for click probe");
    uint pid; DuplicateNative.GetWindowThreadProcessId(candidate, out pid);
    string renderer = Process.GetProcessById((int)pid).MainModule.FileName;
    Require(String.Equals(renderer, Path.Combine(installed, "mozc_renderer.exe"), StringComparison.OrdinalIgnoreCase),
            "Candidate click would target a different renderer: " + renderer);
    DuplicateNative.Rect rect;
    Require(DuplicateNative.GetWindowRect(candidate, out rect) && rect.Right - rect.Left > 40 && rect.Bottom - rect.Top > 20,
            "Invalid candidate window bounds");
    // 標準DPIの第1行内。実ウィンドウへの命中とクリック後の確定を両方要求する。
    var point = new DuplicateNative.Point {X = rect.Left + 30, Y = rect.Top + 12};
    Require(DuplicateNative.WindowFromPoint(point) == candidate, "Candidate first row is obscured");
    row["candidate_window"] = new int[] {rect.Left, rect.Top, rect.Right, rect.Bottom};
    row["renderer"] = renderer;
    Require(DuplicateNative.SetCursorPos(point.X, point.Y), "Cannot position candidate click");
    DuplicateNative.mouse_event(2, 0, 0, 0, UIntPtr.Zero);
    DuplicateNative.mouse_event(4, 0, 0, 0, UIntPtr.Zero);
  }
  void DuplicateTrial(ImeObservation edit, string host, string mode, int delay,
                      List<Dictionary<string, object>> rows) {
    var row = new Dictionary<string, object> {
      {"host", host}, {"mode", mode}, {"space_to_action_ms", delay}, {"passed", false}
    };
    rows.Add(row);
    try {
      Reset(edit); FastType(edit);
      WaitUntil(() => Preedit(edit).Length > 0, "No native composition in duplicate probe");
      row["before_conversion"] = Snapshot(edit);
      if (mode != "enter") RawPress(0x20);
      if (mode == "click" || mode == "down_enter")
        WaitUntil(() => Converted(edit), "No converted preedit before candidate action");
      Thread.Sleep(delay);
      if (mode == "down_enter") Press(0x28);
      if (mode == "click") {
        ClickFirstCandidate(row); Thread.Sleep(180);
        row["after_click"] = Snapshot(edit);
        Require(Results(edit).Length == 0 && Preedit(edit).Length > 0,
                "Candidate selection unexpectedly committed or lost composition");
        RawPress(0x0D);
      }
      else if (mode == "focus") Focus(edit == a ? b : a, true);
      else RawPress(0x0D);
      WaitUntil(() => Results(edit).Length > 0 && Preedit(edit) == "", "No completed native commit in duplicate probe");
      Thread.Sleep(900); // 遅いrefreshを待ってから回数と最終文字列を判定する。
      string[] committed = Results(edit);
      string text = TextOf(edit);
      row["results"] = committed;
      Require(committed.Length == 1, "A single operation delivered " + committed.Length + " native commit results");
      Require(text == committed[0], "Host text differs from the one native result");
      Require(text.Length % 2 != 0 || text.Substring(0, text.Length / 2) != text.Substring(text.Length / 2),
              "The reported whole phrase was duplicated inside one result");
      row["passed"] = true;
    } catch (Exception error) { row["error"] = error.ToString(); throw; }
    finally { row["after"] = Snapshot(edit); Save(); }
  }
  void IntentionalRepeat(ImeObservation edit, string host, List<Dictionary<string, object>> rows) {
    var row = new Dictionary<string, object> {{"host", host}, {"mode", "intentional_repeat"}, {"passed", false}};
    rows.Add(row);
    try {
      Reset(edit);
      for (int i = 0; i < 2; i++) {
        FastType(edit); RawPress(0x75); // F6で同じ読みを明示し、学習による候補差を除く。
        WaitUntil(() => Preedit(edit) == "ごーるにせっていして", "Repeated reading was not delivered");
        RawPress(0x0D);
        int count = i + 1;
        WaitUntil(() => Results(edit).Length >= count && Preedit(edit) == "", "Intentional repeated input was lost");
      }
      Thread.Sleep(900);
      string[] values = Results(edit);
      row["results"] = values;
      Require(values.Length == 2 && values[0] == values[1] && TextOf(edit) == values[0] + values[1],
              "Two intentional equal phrases must remain two commits");
      row["passed"] = true;
    } catch (Exception error) { row["error"] = error.ToString(); throw; }
    finally { row["after"] = Snapshot(edit); Save(); }
  }
  void RunDuplicateStress() {
    var rows = new List<Dictionary<string, object>>();
    var summary = new Dictionary<string, object> {
      {"complete", false}, {"scope", "Native EDIT/RichEdit, CPU GitHub runner; not browsers or GPU hardware"},
      {"trials", rows}
    };
    report["duplicate_stress"] = summary;
    // 利用者の実PCでは、合成クリックで前面窓が外れる環境がある（無関係な対照窓でも再現）。
    // そのPCに限り、クリック試験を合格扱いにせず「未検証」として残し、他の試験を続ける。
    bool skipClick = Environment.GetEnvironmentVariable("KOTORI_LOCAL_ACCEPTANCE") == "owner-approved" &&
        Environment.GetEnvironmentVariable("KOTORI_SKIP_CLICK") == "environment";
    summary["click_skipped"] = skipClick;
    var edits = new ImeObservation[] {a, r};
    for (int i = 0; i < edits.Length; i++) {
      string host = i == 0 ? "EDIT" : "RichEdit";
      foreach (int delay in new int[] {0, 30, 250})
        foreach (string mode in new string[] {"space_enter", "enter", "down_enter", "focus"})
          DuplicateTrial(edits[i], host, mode, delay, rows);
      if (skipClick)
        rows.Add(new Dictionary<string, object> {
          {"host", host}, {"mode", "click"}, {"space_to_action_ms", 0}, {"passed", false},
          {"skipped", "synthetic clicks drop the foreground window on this PC, including an unrelated control window"}
        });
      else DuplicateTrial(edits[i], host, "click", 0, rows);
      IntentionalRepeat(edits[i], host, rows);
    }
    Require(rows.Count == 28, "Incomplete duplicate stress suite");
    summary["complete"] = true;
    Save();
  }
}
