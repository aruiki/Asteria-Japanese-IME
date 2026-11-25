// 紹介動画・README の GIF のための、実際の入力の録画用の窓(docs/marketing/promo-video-v3-requirements.md の「正直さの決まり」)。
//
// ImeBench.cs と同じく、このプロセスだけで指定の IME を有効にし、読みのローマ字を本物のキーとして送る。
// 人の打つ速さ(1 秒に約 6 回)で打ち、Space を 1 回押した直後の第一候補を見せ、Enter で確定する。
// 打った読み・Space の後の文字・確定した文字を記録のファイルに書く(記録と映像が一致しない撮り直しは捨てる)。
//
// 使い方: PromoTyping.exe <CLSID> <プロファイル> <ローマ字> <記録.tsv> [開始の合図のファイル]
//   合図のファイルを渡すと、それができるまで待ってから打ち始める(録画を先に始めるため)。
// 打っている間、この PC のキーボード入力を使う(ほかの操作をしない)。
using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Forms;

class Promo : Form {
  readonly TextBox box = new TextBox();
  readonly string[] args;

  public Promo(string[] a) {
    args = a;
    Text = "無題 - メモ";
    Width = 1100; Height = 150;  // 1 行分。予測の窓は入力中の文字の下(窓の外)に出るので映らない
    StartPosition = FormStartPosition.Manual;
    Left = 80; Top = 80;
    TopMost = true;
    BackColor = System.Drawing.Color.White;
    box.Multiline = true;
    box.BorderStyle = BorderStyle.None;
    box.Dock = DockStyle.Fill;
    box.Font = new System.Drawing.Font("Yu Gothic UI", 20);
    var pad = new Panel { Dock = DockStyle.Fill, Padding = new Padding(28, 22, 28, 22), BackColor = System.Drawing.Color.White };
    pad.Controls.Add(box);
    Controls.Add(pad);
    Shown += (s, e) => { Activate(); box.Focus(); new Thread(Run) { IsBackground = true }.Start(); };
  }

  static void Key(ushort vk, bool up) {
    var ins = new Native.INPUT[1];
    ins[0].type = 1;
    ins[0].ki.wVk = vk;
    ins[0].ki.wScan = (ushort)Native.MapVirtualKeyW(vk, 0);
    ins[0].ki.dwFlags = up ? 2u : 0u;
    Native.SendInput(1, ins, Marshal.SizeOf(typeof(Native.INPUT)));
  }

  static void Press(ushort vk) { Key(vk, false); Key(vk, true); }

  void OnUi(Action a) { Invoke(a); }

  // この窓が前面でなければ止める(ほかの窓にキーを送らない。ImeBench と同じ)。
  void EnsureForeground() {
    IntPtr mine = IntPtr.Zero;
    OnUi(() => { mine = Handle; });
    if (Native.GetForegroundWindow() == mine) return;
    Native.keybd_event(0x12, 0, 0, IntPtr.Zero);
    OnUi(() => { Native.SetForegroundWindow(Handle); Activate(); box.Focus(); });
    Native.keybd_event(0x12, 0, 2, IntPtr.Zero);
    Thread.Sleep(300);
    if (Native.GetForegroundWindow() != mine) throw new Exception("窓を前面にできない(ほかの窓にキーを送らないよう止めた)");
  }

  void Run() {
    string record = args[3];
    try {
      Guid clsid = new Guid(args[0]), profile = new Guid(args[1]);
      OnUi(() => {
        var mgr = (ITfInputProcessorProfileMgr)Activator.CreateInstance(
            Type.GetTypeFromCLSID(new Guid("33C53A50-F456-4884-B049-85FD643ECFED")));
        Guid keyboard = new Guid("34745C63-B2F0-4784-8B67-5E12C8701A31");
        for (int attempt = 0; ; attempt++) {
          int hr = mgr.ActivateProfile(1, 0x0411, ref clsid, ref profile, IntPtr.Zero, 0x10000000 | 0x4 | 0x1);
          if (hr != 0) throw new Exception("ActivateProfile 0x" + hr.ToString("x"));
          TfInputProcessorProfile active;
          if (mgr.GetActiveProfile(ref keyboard, out active) == 0 && active.clsid == clsid &&
              active.guidProfile == profile) break;
          if (attempt >= 4) throw new Exception("指定の IME が有効にならない");
          Thread.Sleep(500);
        }
        IntPtr c = Native.ImmGetContext(box.Handle);
        Native.ImmSetOpenStatus(c, true);
        Native.ImmSetConversionStatus(c, 0x1 | 0x8, 0x8);
        Native.ImmReleaseContext(box.Handle, c);
      });
      // 録画は窓の範囲だけにする(ほかのアプリを映さない)。窓の画面上の位置を書いてから、合図を待つ。
      OnUi(() => {
        RECT r;
        GetWindowRect(Handle, out r);
        File.WriteAllText(record + ".rect", r.Left + " " + r.Top + " " + (r.Right - r.Left) + " " + (r.Bottom - r.Top));
      });
      if (args.Length > 4) {
        while (!File.Exists(args[4])) Thread.Sleep(100);
      }
      Thread.Sleep(1200);  // 録画の頭の静止
      EnsureForeground();
      var rnd = new Random(7);
      foreach (char ch in args[2]) {
        short r = Native.VkKeyScanW(ch);
        if (r == -1) continue;
        Press((ushort)(r & 0xff));
        Thread.Sleep(120 + rnd.Next(70));  // 1 秒に約 6 回
      }
      Thread.Sleep(700);
      EnsureForeground();
      Press(0x20);  // Space 1 回
      Thread.Sleep(1800);
      string shown = "";
      OnUi(() => { shown = box.Text; });
      EnsureForeground();
      Press(0x0D);  // Enter
      Thread.Sleep(1500);
      string got = "";
      OnUi(() => { got = box.Text; });
      File.WriteAllText(record, "romaji\t" + args[2] + "\nafter_space\t" + shown + "\ncommitted\t" + got + "\n",
                        new UTF8Encoding(false));
    } catch (Exception e) {
      File.WriteAllText(record + ".error.txt", e.ToString());
    }
    OnUi(Close);
  }

  [StructLayout(LayoutKind.Sequential)] struct RECT { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] static extern bool SetProcessDPIAware();

  [STAThread]
  static void Main(string[] a) {
    SetProcessDPIAware();  // 窓の位置を実際の画素で得る(録画の範囲に使う)
    Application.EnableVisualStyles();
    Application.Run(new Promo(a));
  }
}
