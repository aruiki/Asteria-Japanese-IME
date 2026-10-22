// 候補窓のクリックで確定まで進む原因を調べる単独試験（2026-10-22、利用者の実PCでの失敗）。
// クリック前後の候補窓の画像、前面窓・フォーカス・キャプチャ、IMEのイベントを時刻つきで残す。
// 合否は付けない。local_install_check.py clickprobe からだけ実行する。
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

static class ProbeNative {
  [StructLayout(LayoutKind.Sequential)]
  public struct GuiThreadInfo {
    public int cbSize, flags;
    public IntPtr active, focus, capture, menuOwner, moveSize, caret;
    public DuplicateNative.Rect caretRect;
  }
  [DllImport("user32.dll")] public static extern bool GetGUIThreadInfo(uint thread, ref GuiThreadInfo info);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)]
  public static extern int GetClassNameW(IntPtr window, StringBuilder text, int maximum);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
}

sealed partial class Acceptance {
  static Dictionary<string, object> Describe(IntPtr window) {
    var result = new Dictionary<string, object> {{"handle", window.ToInt64()}};
    if (window == IntPtr.Zero) return result;
    var name = new StringBuilder(128); ProbeNative.GetClassNameW(window, name, name.Capacity);
    result["class"] = name.ToString();
    uint pid; DuplicateNative.GetWindowThreadProcessId(window, out pid);
    result["pid"] = (long)pid;
    try { result["process"] = Process.GetProcessById((int)pid).ProcessName; } catch (Exception) { }
    return result;
  }
  Dictionary<string, object> Sample(string label, Stopwatch clock) {
    var info = new ProbeNative.GuiThreadInfo();
    info.cbSize = Marshal.SizeOf(typeof(ProbeNative.GuiThreadInfo));
    ProbeNative.GetGUIThreadInfo(0, ref info);
    IntPtr ours = IntPtr.Zero; Ui(() => ours = a.Edit.Handle);
    return new Dictionary<string, object> {
      {"label", label}, {"ms", clock.ElapsedMilliseconds},
      {"foreground", Describe(ProbeNative.GetForegroundWindow())},
      {"focus", Describe(info.focus)}, {"capture", Describe(info.capture)},
      {"focus_is_probe_edit", info.focus == ours},
      {"candidate_visible", DuplicateNative.VisibleCandidate() != IntPtr.Zero},
      {"preedit", Preedit(a)}, {"results", Results(a)}, {"text", TextOf(a)}
    };
  }
  string CaptureScreen(DuplicateNative.Rect rect, string name) {
    string path = Path.Combine(Path.GetDirectoryName(Path.GetFullPath(destination)), name);
    using (var image = new Bitmap(rect.Right - rect.Left, rect.Bottom - rect.Top))
    using (var graphics = Graphics.FromImage(image)) {
      graphics.CopyFromScreen(rect.Left, rect.Top, 0, 0, image.Size);
      image.Save(path, ImageFormat.Png);
    }
    return path;
  }
  // 対照: 候補窓と同じWS_EX_NOACTIVATE|TOPMOST|TOOLWINDOWの空の窓。Asteria以外でも
  // クリックで前面窓が外れるなら、原因はこのPCの常駐ソフト等の環境側にある。
  sealed class NoActivateWindow : System.Windows.Forms.Form {
    protected override bool ShowWithoutActivation { get { return true; } }
    protected override System.Windows.Forms.CreateParams CreateParams {
      get { var p = base.CreateParams; p.ExStyle |= 0x08000000 | 0x8 | 0x80; return p; }
    }
    protected override void WndProc(ref System.Windows.Forms.Message m) {
      if (m.Msg == 0x21) { m.Result = (IntPtr)3; return; } // WM_MOUSEACTIVATE → MA_NOACTIVATE
      base.WndProc(ref m);
    }
  }
  void RunControlClick() {
    var row = new Dictionary<string, object> {{"trial", "control_noactivate_window"}};
    var samples = new List<Dictionary<string, object>>();
    row["samples"] = samples;
    ((List<Dictionary<string, object>>)report["click_probe"]).Add(row);
    NoActivateWindow window = null;
    try {
      var clock = Stopwatch.StartNew();
      Ui(() => {
        window = new NoActivateWindow {
          FormBorderStyle = System.Windows.Forms.FormBorderStyle.None, StartPosition = System.Windows.Forms.FormStartPosition.Manual,
          Left = Left + Width + 20, Top = Top, Width = 160, Height = 200, BackColor = Color.LightYellow
        };
        window.Show();
      });
      Reset(a); FastType(a);
      WaitUntil(() => Preedit(a).Length > 0, "No composition");
      RawPress(0x20);
      WaitUntil(() => Converted(a), "No converted preedit");
      Thread.Sleep(300);
      samples.Add(Sample("before_click", clock));
      var point = new DuplicateNative.Point();
      Ui(() => { point.X = window.Left + 30; point.Y = window.Top + 12; });
      row["window_from_point"] = Describe(DuplicateNative.WindowFromPoint(point));
      Require(DuplicateNative.SetCursorPos(point.X, point.Y), "Cannot position cursor");
      Thread.Sleep(50);
      DuplicateNative.mouse_event(2, 0, 0, 0, UIntPtr.Zero);
      samples.Add(Sample("down_0", clock)); Thread.Sleep(30);
      DuplicateNative.mouse_event(4, 0, 0, 0, UIntPtr.Zero);
      Thread.Sleep(180); samples.Add(Sample("up_180", clock));
      row["committed_by_click"] = Results(a).Length > 0;
    } catch (Exception error) { row["error"] = error.ToString(); }
    finally {
      if (window != null) Ui(() => window.Close());
      Focus(a, true); Press(0x1B); Press(0x1B);
      Save();
    }
  }
  partial void RunClickProbe() {
    var trials = new List<Dictionary<string, object>>();
    report["click_probe"] = trials;
    RunControlClick();
    // 0: 元の試験と同じ12px。1: 行の中央付近（+40px）。2: 一覧を開いてから1秒待つ（AIの後続更新を除く）。
    int[] offsets = {12, 40, 12};
    int[] settle = {0, 0, 1000};
    for (int t = 0; t < offsets.Length; t++) {
      var row = new Dictionary<string, object> {{"trial", t}, {"y_offset", offsets[t]}, {"settle_ms", settle[t]}};
      var samples = new List<Dictionary<string, object>>();
      row["samples"] = samples;
      trials.Add(row);
      try {
        var clock = Stopwatch.StartNew();
        Reset(a); FastType(a);
        WaitUntil(() => Preedit(a).Length > 0, "No composition");
        RawPress(0x20);
        WaitUntil(() => Converted(a), "No converted preedit");
        samples.Add(Sample("after_space", clock));
        row["list_visible_after_space"] = DuplicateNative.VisibleCandidate() != IntPtr.Zero;
        if (DuplicateNative.VisibleCandidate() == IntPtr.Zero) RawPress(0x28);
        IntPtr candidate = IntPtr.Zero;
        WaitUntil(() => (candidate = DuplicateNative.VisibleCandidate()) != IntPtr.Zero, "No candidate window");
        Thread.Sleep(150 + settle[t]);
        DuplicateNative.Rect rect;
        Require(DuplicateNative.GetWindowRect(candidate, out rect), "No candidate rect");
        row["candidate_window"] = new int[] {rect.Left, rect.Top, rect.Right, rect.Bottom};
        row["image_before"] = CaptureScreen(rect, "click-probe-" + t + "-before.png");
        samples.Add(Sample("before_click", clock));
        var point = new DuplicateNative.Point {X = rect.Left + 30, Y = rect.Top + offsets[t]};
        row["point"] = new int[] {point.X, point.Y};
        row["window_from_point"] = Describe(DuplicateNative.WindowFromPoint(point));
        Require(DuplicateNative.SetCursorPos(point.X, point.Y), "Cannot position cursor");
        Thread.Sleep(50);
        DuplicateNative.mouse_event(2, 0, 0, 0, UIntPtr.Zero);
        samples.Add(Sample("down_0", clock)); Thread.Sleep(30); samples.Add(Sample("down_30", clock));
        DuplicateNative.mouse_event(4, 0, 0, 0, UIntPtr.Zero);
        int elapsed = 0;
        foreach (int wait in new int[] {0, 30, 80, 180, 400}) {
          Thread.Sleep(wait - elapsed); elapsed = wait; samples.Add(Sample("up_" + wait, clock));
        }
        if (DuplicateNative.VisibleCandidate() != IntPtr.Zero && DuplicateNative.GetWindowRect(candidate, out rect))
          row["image_after"] = CaptureScreen(rect, "click-probe-" + t + "-after.png");
        row["committed_by_click"] = Results(a).Length > 0;
      } catch (Exception error) { row["error"] = error.ToString(); }
      finally {
        // 次の試行に未確定文字を残さない。確定済みならEscは何もしない。
        Focus(a, true); Press(0x1B); Press(0x1B);
        Save();
      }
    }
  }
}
