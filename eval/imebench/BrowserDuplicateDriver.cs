// 使い捨てCIのEdgeへ実SendInputを送る。DevToolsによる文字入力は使わない。
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Web.Script.Serialization;

static class BrowserDuplicateDriver {
  delegate bool WindowCallback(IntPtr window, IntPtr parameter);
  [DllImport("user32.dll")] static extern bool EnumWindows(WindowCallback callback, IntPtr parameter);
  static void Require(bool value, string error) { if (!value) throw new Exception(error); }
  static void Key(ushort key, bool up) {
    var input = new Native.INPUT[1]; input[0].type = 1;
    input[0].ki.wVk = key; input[0].ki.wScan = (ushort)Native.MapVirtualKeyW(key, 0);
    input[0].ki.dwFlags = up ? 2u : 0u;
    Require(Native.SendInput(1, input, Marshal.SizeOf(typeof(Native.INPUT))) == 1, "SendInput failed");
  }
  static void Press(ushort key) { Key(key, false); Key(key, true); }
  [STAThread] static int Main(string[] args) {
    // 使い捨てCI、または利用者本人が実PCでの検証を許可したlocal_install_check.pyからだけ実行する。
    bool runner = Environment.GetEnvironmentVariable("GITHUB_ACTIONS") == "true" &&
        Environment.GetEnvironmentVariable("KOTORI_DISPOSABLE_RUNNER") == "github-hosted";
    bool ownerApproved = Environment.GetEnvironmentVariable("KOTORI_LOCAL_ACCEPTANCE") == "owner-approved";
    if (args.Length != 4 || !(runner || ownerApproved)) return 2;
    try {
      string marker = args[0], mode = args[1], installed = Path.GetFullPath(args[3]);
      int delay = int.Parse(args[2]), candidateMoves = 0;
      Require(marker.StartsWith("AsteriaDuplicateProbe-") && marker.Length > 30, "Invalid test window marker");
      IntPtr edge = IntPtr.Zero;
      var windowWait = Stopwatch.StartNew();
      while (edge == IntPtr.Zero && windowWait.ElapsedMilliseconds < 10000) {
        EnumWindows((window, unused) => {
          var title = new StringBuilder(512); AcceptanceNative.GetWindowTextW(window, title, title.Capacity);
          if (title.ToString().Contains(marker) && DuplicateNative.IsWindowVisible(window)) {
            edge = window; return false;
          }
          return true;
        }, IntPtr.Zero);
        if (edge == IntPtr.Zero) Thread.Sleep(50);
      }
      Require(edge != IntPtr.Zero, "Disposable browser window was not found");
      uint pid; DuplicateNative.GetWindowThreadProcessId(edge, out pid);
      var process = Process.GetProcessById((int)pid);
      Require(String.Equals(Path.GetFileName(process.MainModule.FileName), "msedge.exe", StringComparison.OrdinalIgnoreCase),
              "Probe target is not Edge");
      if (mode == "activate") {
        Guid clsid = new Guid("2A1ADAE4-8061-4AC0-A7D4-713BF8DCB591");
        Guid profile = new Guid("455A87E7-275F-4582-92CB-6600917A436D");
        var manager = (ITfInputProcessorProfileMgr)Activator.CreateInstance(
          Type.GetTypeFromCLSID(new Guid("33C53A50-F456-4884-B049-85FD643ECFED")));
        Require(manager.ActivateProfile(1, 0x0411, ref clsid, ref profile, IntPtr.Zero, 0x10000005) == 0,
                "Session TIP activation failed");
        Native.SetForegroundWindow(edge); Thread.Sleep(500);
        IntPtr context = Native.ImmGetContext(edge);
        if (context != IntPtr.Zero) {
          Native.ImmSetOpenStatus(context, true); Native.ImmSetConversionStatus(context, 9, 8);
          Native.ImmReleaseContext(edge, context);
        }
        Press(0xF2); Thread.Sleep(300); // ひらがなを明示する。
      } else {
        Require(AcceptanceNative.GetForegroundWindow() == edge, "Browser lost foreground before input");
        if (mode == "type") {
          foreach (char key in "go-runisetteishite") {
            Require(AcceptanceNative.GetForegroundWindow() == edge, "Browser lost foreground while typing");
            short mapped = Native.VkKeyScanW(key);
            Require(mapped != -1 && (mapped & 0xff00) == 0, "Unexpected fixed-key mapping");
            Press((ushort)mapped); Thread.Sleep(5);
          }
        } else if (mode == "space_enter") { Press(0x20); Thread.Sleep(delay); Press(0x0D); }
        else if (mode == "down_enter") { Press(0x28); Thread.Sleep(delay); Press(0x0D); }
        else if (mode == "open_candidates") {
          // 候補がひらがなのままでも正常。文字列の変化を完了条件にしない。
          while (DuplicateNative.VisibleCandidate() == IntPtr.Zero && candidateMoves < 60) {
            Require(AcceptanceNative.GetForegroundWindow() == edge, "Browser lost foreground while opening candidates");
            Press(0x28); candidateMoves++; Thread.Sleep(250);
          }
          IntPtr candidate = DuplicateNative.VisibleCandidate();
          Require(candidate != IntPtr.Zero, "Candidate list did not open");
          uint rendererPid; DuplicateNative.GetWindowThreadProcessId(candidate, out rendererPid);
          Require(String.Equals(Process.GetProcessById((int)rendererPid).MainModule.FileName,
                                Path.Combine(installed, "mozc_renderer.exe"), StringComparison.OrdinalIgnoreCase),
                  "Candidate list belongs to another renderer");
        }
        else if (mode == "space") Press(0x20);
        else if (mode == "enter") Press(0x0D);
        else if (mode == "escape") { Press(0x1B); Thread.Sleep(100); Press(0x1B); }
        else if (mode == "f6_enter") { Press(0x75); Thread.Sleep(100); Press(0x0D); }
        else if (mode == "click") {
          IntPtr candidate = DuplicateNative.VisibleCandidate();
          Require(candidate != IntPtr.Zero, "Candidate is not visible");
          uint rendererPid; DuplicateNative.GetWindowThreadProcessId(candidate, out rendererPid);
          string renderer = Process.GetProcessById((int)rendererPid).MainModule.FileName;
          Require(String.Equals(renderer, Path.Combine(installed, "mozc_renderer.exe"), StringComparison.OrdinalIgnoreCase),
                  "Unexpected candidate renderer");
          DuplicateNative.Rect rect;
          Require(DuplicateNative.GetWindowRect(candidate, out rect), "No candidate bounds");
          var point = new DuplicateNative.Point {X = rect.Left + 30, Y = rect.Top + 12};
          Require(DuplicateNative.WindowFromPoint(point) == candidate, "Candidate click is obscured");
          Require(DuplicateNative.SetCursorPos(point.X, point.Y), "Cannot position click");
          DuplicateNative.mouse_event(2, 0, 0, 0, UIntPtr.Zero);
          DuplicateNative.mouse_event(4, 0, 0, 0, UIntPtr.Zero);
        } else throw new Exception("Unknown probe mode: " + mode);
      }
      var tips = new List<string>();
      foreach (ProcessModule module in process.Modules)
        if (String.Equals(module.ModuleName, "mozc_tip64.dll", StringComparison.OrdinalIgnoreCase)) tips.Add(module.FileName);
      Console.WriteLine(new JavaScriptSerializer().Serialize(new {mode = mode, process_id = pid, loaded_tips = tips, candidate_moves = candidateMoves}));
      return 0;
    } catch (Exception error) { Console.Error.WriteLine(error.ToString()); return 1; }
  }
}
