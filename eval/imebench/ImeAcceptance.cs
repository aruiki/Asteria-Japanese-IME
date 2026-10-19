// MSIで導入したMozc TIPの入力・取消・候補移動を、使い捨てWindows CIで確認する。
// ImeBench.csのNative/ITfInputProcessorProfileMgrを共有する。CLI変換器は使わない。
// 実行はinstaller_smoke.py --acceptanceを通す。利用者のデスクトップでは実行しない。
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Web.Script.Serialization;
using System.Windows.Forms;

static class AcceptanceNative {
  [DllImport("imm32.dll", CharSet = CharSet.Unicode)]
  public static extern int ImmGetCompositionStringW(IntPtr context, uint index, byte[] buffer, uint length);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll", CharSet = CharSet.Unicode)]
  public static extern int GetWindowTextW(IntPtr window, StringBuilder text, int maximum);
}

sealed class ImeObservation : NativeWindow {
  public readonly Control Edit;
  public string Composition = "";
  public byte[] Attributes = new byte[0];
  public readonly List<Dictionary<string, object>> Events = new List<Dictionary<string, object>>();
  public ImeObservation(Control edit) { Edit = edit; AssignHandle(edit.Handle); }
  byte[] ReadBytes(uint index) {
    IntPtr context = Native.ImmGetContext(Edit.Handle);
    if (context == IntPtr.Zero) return new byte[0];
    try {
      int size = AcceptanceNative.ImmGetCompositionStringW(context, index, null, 0);
      if (size <= 0) return new byte[0];
      if (size > 8192) throw new Exception("Invalid IME composition size: " + size);
      byte[] buffer = new byte[size];
      int got = AcceptanceNative.ImmGetCompositionStringW(context, index, buffer, (uint)size);
      if (got != size) throw new Exception("IME composition changed while reading");
      return buffer;
    } finally { Native.ImmReleaseContext(Edit.Handle, context); }
  }
  string Read(uint index) {
    byte[] buffer = ReadBytes(index);
    if (buffer.Length % 2 != 0) throw new Exception("Invalid UTF-16 IME composition size");
    return Encoding.Unicode.GetString(buffer);
  }
  protected override void WndProc(ref Message message) {
    if (message.Msg == 0x10F) { // WM_IME_COMPOSITION。確定結果はホストによる処理の前に採取する。
      long flags = message.LParam.ToInt64();
      if ((flags & 8) != 0) Composition = Read(8); // GCS_COMPSTR
      if ((flags & 16) != 0) Attributes = ReadBytes(16); // GCS_COMPATTR
      Events.Add(new Dictionary<string, object> {
        {"message", "composition"}, {"flags", flags}, {"preedit", Composition}, {"attributes", Attributes},
        {"result", (flags & 0x800) != 0 ? Read(0x800) : ""}
      });
    }
    if (message.Msg == 0x10E) { // WM_IME_ENDCOMPOSITION
      Composition = "";
      Attributes = new byte[0];
      Events.Add(new Dictionary<string, object> {{"message", "end"}});
    }
    base.WndProc(ref message);
  }
}

sealed class Acceptance : Form {
  readonly TextBox first = new TextBox(), second = new TextBox(), password = new TextBox();
  readonly RichTextBox rich = new RichTextBox();
  readonly Dictionary<string, object> report = new Dictionary<string, object>();
  readonly List<Dictionary<string, object>> checks = new List<Dictionary<string, object>>();
  readonly string destination, installed;
  ImeObservation a, b, r, p;
  int exit = 1;

  Acceptance(string[] args) {
    installed = Path.GetFullPath(args[0]); destination = args[1];
    Text = "Asteria installed TIP acceptance"; Width = 900; Height = 420; TopMost = true;
    first.SetBounds(20, 20, 820, 45); second.SetBounds(20, 85, 820, 45);
    rich.SetBounds(20, 150, 820, 120); password.SetBounds(20, 290, 820, 45);
    password.UseSystemPasswordChar = true;
    foreach (Control control in new Control[] {first, second, rich, password}) {
      control.Font = new System.Drawing.Font("Yu Gothic UI", 14); Controls.Add(control);
    }
    Shown += (sender, e) => {
      a = new ImeObservation(first); b = new ImeObservation(second);
      r = new ImeObservation(rich); p = new ImeObservation(password);
      new Thread(Run) {IsBackground = true}.Start();
    };
  }
  void Ui(Action action) { Invoke(action); }
  void Save() {
    report["checks"] = checks;
    File.WriteAllText(destination, new JavaScriptSerializer().Serialize(report), new UTF8Encoding(false));
  }
  static void Require(bool condition, string message) { if (!condition) throw new Exception(message); }
  static void Key(ushort key, bool up) {
    var input = new Native.INPUT[1]; input[0].type = 1;
    input[0].ki.wVk = key; input[0].ki.wScan = (ushort)Native.MapVirtualKeyW(key, 0);
    input[0].ki.dwFlags = up ? 2u : 0u;
    Require(Native.SendInput(1, input, Marshal.SizeOf(typeof(Native.INPUT))) == 1,
            "SendInput failed: " + Marshal.GetLastWin32Error());
  }
  static void Press(ushort key) { Key(key, false); Key(key, true); Thread.Sleep(180); }
  void Type(ImeObservation edit, string text) {
    Ui(() => Require(edit.Edit.ContainsFocus && AcceptanceNative.GetForegroundWindow() == Handle,
                     "Native input target is not focused"));
    foreach (char ch in text) {
      Require(ch >= 'a' && ch <= 'z' || ch >= '0' && ch <= '9', "Only fixed ASCII test keys are permitted");
      short mapped = Native.VkKeyScanW(ch);
      Require(mapped != -1 && (mapped & 0xff00) == 0, "Unexpected keyboard mapping");
      Key((ushort)mapped, false); Key((ushort)mapped, true); Thread.Sleep(18);
    }
    Thread.Sleep(250);
  }
  void Focus(ImeObservation edit, bool enableIme) {
    Ui(() => {
      Activate(); Native.SetForegroundWindow(Handle); edit.Edit.Focus();
      Require(edit.Edit.ContainsFocus, "Control did not receive focus");
      if (enableIme) {
        IntPtr context = Native.ImmGetContext(edit.Edit.Handle);
        Require(context != IntPtr.Zero, "No IMM context for ordinary edit control");
        try {
          Require(Native.ImmSetOpenStatus(context, true), "Cannot enable IME");
          Require(Native.ImmSetConversionStatus(context, 9, 8), "Cannot select Hiragana mode");
        } finally { Native.ImmReleaseContext(edit.Edit.Handle, context); }
      }
    });
    Thread.Sleep(180);
  }
  void Reset(ImeObservation edit) {
    Focus(edit, true); Press(0x1B); Press(0x1B);
    Ui(() => { edit.Edit.Text = ""; edit.Composition = ""; edit.Attributes = new byte[0]; edit.Events.Clear(); });
  }
  Dictionary<string, object> Snapshot(ImeObservation edit) {
    Dictionary<string, object> result = null;
    Ui(() => result = new Dictionary<string, object> {
      {"text", ReadText(edit.Edit)}, {"preedit", edit.Composition},
      {"events", edit.Events.ToArray()}
    });
    return result;
  }
  static string ReadText(Control edit) {
    // RichTextBox.TextのStreamOutは未確定文字を確定させることがある。
    // 観測で入力を変えないよう、ホストのWM_GETTEXT経路を使う。
    var text = new StringBuilder(8192);
    AcceptanceNative.GetWindowTextW(edit.Handle, text, text.Capacity);
    return text.ToString();
  }
  string Preedit(ImeObservation edit) { string text = ""; Ui(() => text = edit.Composition); return text; }
  string TextOf(ImeObservation edit) { string text = ""; Ui(() => text = ReadText(edit.Edit)); return text; }
  bool Converted(ImeObservation edit) {
    bool converted = false;
    Ui(() => {
      // ひらがな候補も変換済みになり得る。文字列が変わることを条件にしない。
      foreach (byte attribute in edit.Attributes)
        if (attribute == 1 || attribute == 2 || attribute == 5) converted = true;
    });
    return converted;
  }
  void WaitUntil(Func<bool> condition, string message) {
    // AIの非同期配送を観測する。固定sleepの直後を完成扱いにしない。
    var timer = Stopwatch.StartNew();
    while (!condition()) {
      Require(timer.ElapsedMilliseconds < 15000, message);
      Thread.Sleep(25);
    }
  }
  void Check(string name, Action<Dictionary<string, object>> action) {
    var row = new Dictionary<string, object> {{"name", name}, {"passed", false}};
    checks.Add(row);
    try { action(row); row["passed"] = true; }
    catch (Exception error) { row["error"] = error.ToString(); throw; }
    finally { row["first"] = Snapshot(a); row["second"] = Snapshot(b); row["rich"] = Snapshot(r); Save(); }
  }
  void Convert(ImeObservation edit, string keys, string expected) {
    Reset(edit); Type(edit, keys);
    Require(Preedit(edit).Length > 0, "No native preedit observed before Space");
    Press(0x20);
    WaitUntil(() => Preedit(edit) == expected, "Converted preedit was not delivered: " + expected);
    Press(0x0D);
    WaitUntil(() => TextOf(edit) == expected && Preedit(edit) == "", "Committed text differs from displayed preedit");
  }
  void Run() {
    report["complete"] = false; report["native_tsf"] = true;
    report["scope"] = "Native WinForms EDIT/RichEdit controls; not Notepad, browsers, Office or physical low-end hardware";
    report["secure_field_internal_ai_or_learning_verified"] = false;
    try {
      Ui(() => {
        Guid clsid = new Guid("2A1ADAE4-8061-4AC0-A7D4-713BF8DCB591");
        Guid profile = new Guid("455A87E7-275F-4582-92CB-6600917A436D");
        var manager = (ITfInputProcessorProfileMgr)Activator.CreateInstance(
          System.Type.GetTypeFromCLSID(new Guid("33C53A50-F456-4884-B049-85FD643ECFED")));
        int hr = manager.ActivateProfile(1, 0x0411, ref clsid, ref profile, IntPtr.Zero, 0x10000005);
        Require(hr == 0, "ActivateProfile failed: 0x" + hr.ToString("x"));
        report["activation_hresult"] = hr;
      });
      Thread.Sleep(1500);
      Check("plain_preedit_space_commit", row => Convert(a, "nihonngo", "日本語"));
      string tip = "";
      foreach (ProcessModule module in Process.GetCurrentProcess().Modules)
        if (String.Equals(module.ModuleName, "mozc_tip64.dll", StringComparison.OrdinalIgnoreCase)) tip = module.FileName;
      Require(String.Equals(tip, Path.Combine(installed, "mozc_tip64.dll"), StringComparison.OrdinalIgnoreCase),
              "The installed Asteria TIP was not loaded in the input host: " + tip);
      report["loaded_tip"] = tip;
      Check("richedit_preedit_space_commit", row => Convert(r, "toukyou", "東京"));
      Check("candidate_down_page_roundtrip_commit", row => {
        Reset(a); Type(a, "hashi"); Press(0x20);
        WaitUntil(() => Preedit(a).Length > 0 && Converted(a), "Converted candidates were not delivered");
        string initial = Preedit(a); Require(initial.Length > 0, "No converted preedit");
        Press(0x28); string down = Preedit(a);
        Press(0x22); string next = Preedit(a);
        Press(0x21); string previous = Preedit(a);
        row["selection"] = new string[] {initial, down, next, previous};
        Require(down != initial && next != down, "Candidate movement did not change the selected preedit");
        Require(previous == initial, "PageUp did not return to the first candidate");
        Press(0x0D); Require(TextOf(a) == previous && Preedit(a) == "", "Selected candidate differs from commit");
      });
      Check("cancel_and_resume", row => {
        Reset(a); Type(a, "toukyou"); Press(0x20);
        Require(Preedit(a).Length > 0, "Cancellation control had no active conversion");
        Press(0x1B); row["first_escape"] = Snapshot(a); Press(0x1B);
        Require(Preedit(a) == "" && TextOf(a) == "", "Cancel left composition or committed text");
        Convert(a, "nihonngo", "日本語");
      });
      Check("backspace_and_continue", row => {
        Reset(a); Type(a, "kana"); Require(Preedit(a) == "かな", "Missing initial preedit");
        Press(0x08); Require(Preedit(a) == "か", "Backspace did not remove one kana");
        Type(a, "na"); Press(0x75); Press(0x0D); Require(TextOf(a) == "かな", "Input after Backspace failed");
      });
      Check("f6_to_f10", row => {
        string[] expected = {"かな", "カナ", "ｶﾅ", "ｋａｎａ", "kana"};
        var got = new List<string>();
        for (int i = 0; i < 5; i++) {
          Reset(a); Type(a, "kana"); Press((ushort)(0x75 + i));
          Require(Preedit(a) == expected[i], "Function key preedit mismatch at F" + (6 + i));
          Press(0x0D); got.Add(TextOf(a)); Require(TextOf(a) == expected[i], "Function key commit mismatch");
        }
        row["committed"] = got;
      });
      Check("focus_between_ordinary_fields", row => {
        Reset(b); Reset(a); Type(a, "nihonngo");
        Require(Preedit(a) == "にほんご", "Focus control had no composition");
        Focus(b, true);
        Require(Preedit(a) == "" && TextOf(a) == "にほんご", "Focus loss lost or retained uncommitted composition");
        Require(TextOf(b) == "" && Preedit(b) == "", "Composition leaked into the next field");
        Type(b, "toukyou"); Press(0x20); Press(0x0D);
        WaitUntil(() => TextOf(b) == "東京" && TextOf(a) == "にほんご", "Focus destination input failed");
        Focus(a, true); Require(Preedit(a) == "", "Old composition resumed after refocusing");
      });
      Check("password_host_visible_behavior", row => {
        // 合成した公開文字列だけ。欄内部のAI停止/非学習を画面出力だけで証明しない。
        Focus(p, false); Ui(() => password.Clear()); Type(p, "abcdef12");
        row["password_field"] = Snapshot(p);
        Require(TextOf(p) == "abcdef12" && Preedit(p) == "" && p.Events.Count == 0,
                "Password host accepted composition instead of literal ASCII keys");
        Convert(a, "nihonngo", "日本語");
      });
      Require(checks.Count == 8, "Incomplete acceptance suite");
      report["complete"] = true; exit = 0;
    } catch (Exception error) { report["error"] = error.ToString(); }
    finally { Save(); Ui(Close); }
  }
  [STAThread] static int Main(string[] args) {
    if (args.Length != 2 || Environment.GetEnvironmentVariable("GITHUB_ACTIONS") != "true" ||
        Environment.GetEnvironmentVariable("KOTORI_DISPOSABLE_RUNNER") != "github-hosted") return 2;
    Application.EnableVisualStyles(); var form = new Acceptance(args); Application.Run(form); return form.exit;
  }
}
