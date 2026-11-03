// 実際の IME で、入力中(Space を押す前)に候補の窓が出るかを確かめる(docs/adr/0081)。
//
// テキストボックスの窓を前に出し、指定の IME をこのプロセスで有効にして、読みのローマ字をキーとして送る。
// 打ち終わってから手を止めた間に候補の窓(KotoriCandidateWindow)が見えたか、Space の後に見えたかと、
// Space の後の未確定の文字を記録し、Esc で取り消す(確定しないので学習しない)。
//
// 使い方: ImeWindowProbe.exe <テキストサービスの CLSID> <プロファイルの GUID> <入力.tsv> <出力.tsv> [手を止める ms] [space2]
//   入力.tsv: 1 行に「番号<TAB>ローマ字」。
//   出力.tsv: 1 行に「番号<TAB>止めた間に窓(0/1)<TAB>Space の後に窓(0/1)<TAB>Space の後の表示」。
//   space2 を付けると、Space を 2 回押した後(候補の一覧が開くはず)の窓を記録する。
// 計測の間、この PC のキーボード入力を使う(ほかの操作をしない)。
using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Forms;

[ComImport, Guid("71C6E74C-0F28-11D8-A82A-00065B84435C"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface ITfInputProcessorProfileMgr {
  [PreserveSig] int ActivateProfile(uint dwProfileType, ushort langid, ref Guid clsid, ref Guid guidProfile,
                                    IntPtr hkl, uint dwFlags);
}

static class Native {
  [StructLayout(LayoutKind.Sequential)]
  public struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Sequential)]
  public struct INPUT { public uint type; public KEYBDINPUT ki; public long pad; }
  public delegate bool WindowCallback(IntPtr window, IntPtr parameter);
  [DllImport("user32.dll", SetLastError = true)] public static extern uint SendInput(uint n, INPUT[] inputs, int size);
  [DllImport("user32.dll")] public static extern short VkKeyScanW(char ch);
  [DllImport("user32.dll")] public static extern uint MapVirtualKeyW(uint code, uint type);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool EnumWindows(WindowCallback callback, IntPtr parameter);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder name, int size);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("imm32.dll")] public static extern IntPtr ImmGetContext(IntPtr h);
  [DllImport("imm32.dll")] public static extern bool ImmReleaseContext(IntPtr h, IntPtr c);
  [DllImport("imm32.dll")] public static extern bool ImmSetOpenStatus(IntPtr c, bool open);
  [DllImport("imm32.dll")] public static extern bool ImmSetConversionStatus(IntPtr c, uint conv, uint sentence);

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

class Probe : Form {
  readonly TextBox box = new TextBox();
  readonly TextBox other = new TextBox();
  readonly string[] args;
  public Probe(string[] a) {
    args = a;
    Text = "ImeWindowProbe";
    Width = 900; Height = 140;
    TopMost = true;
    box.Dock = DockStyle.Fill;
    box.Font = new System.Drawing.Font("Yu Gothic UI", 14);
    other.Dock = DockStyle.Bottom;
    Controls.Add(box);
    Controls.Add(other);
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

  static void TypeKeys(string romaji, int keyMs) {
    foreach (char ch in romaji) {
      short r = Native.VkKeyScanW(ch);
      if (r == -1) continue;
      bool shift = (r & 0x100) != 0;
      if (shift) Key(0x10, false);
      Press((ushort)(r & 0xff));
      if (shift) Key(0x10, true);
      Thread.Sleep(keyMs);
    }
  }

  // ms の間、100 ms ごとに候補の窓を見て、一度でも見えたら true。
  static bool SeenWithin(int ms) {
    bool seen = false;
    for (int t = 0; t < ms; t += 100) {
      seen |= Native.CandidateVisible();
      Thread.Sleep(100);
    }
    return seen | Native.CandidateVisible();
  }

  void OnUi(Action a) { Invoke(a); }

  void Run() {
    try {
      Guid clsid = new Guid(args[0]), profile = new Guid(args[1]);
      int pause = args.Length > 4 ? int.Parse(args[4]) : 2500;
      OnUi(() => {
        var mgr = (ITfInputProcessorProfileMgr)Activator.CreateInstance(
            Type.GetTypeFromCLSID(new Guid("33C53A50-F456-4884-B049-85FD643ECFED")));
        int hr = mgr.ActivateProfile(1, 0x0411, ref clsid, ref profile, IntPtr.Zero, 0x10000000 | 0x4 | 0x1);
        if (hr != 0) throw new Exception("ActivateProfile 0x" + hr.ToString("x"));
      });
      Thread.Sleep(1500);
      using (var w = new StreamWriter(args[3], false, new UTF8Encoding(false))) {
        foreach (string line in File.ReadAllLines(args[2], Encoding.UTF8)) {
          string[] f = line.Split('\t');
          if (f.Length < 2) continue;
          OnUi(() => { other.Focus(); });
          Thread.Sleep(150);
          OnUi(() => {
            box.Clear();
            Native.SetForegroundWindow(Handle);
            box.Focus();
            IntPtr c = Native.ImmGetContext(box.Handle);
            Native.ImmSetOpenStatus(c, true);
            Native.ImmSetConversionStatus(c, 0x1 | 0x8, 0x8);  // ひらがな・全角、一般
            Native.ImmReleaseContext(box.Handle, c);
          });
          Thread.Sleep(150);
          TypeKeys(f[1], 40);
          bool paused = SeenWithin(pause);
          Press(0x20);  // Space(変換)
          bool afterSpace = SeenWithin(1500);
          // 第 6 引数が space2 なら、もう一度 Space を押して候補の一覧が開くかを見る。
          if (args.Length > 5 && args[5] == "space2") {
            Press(0x20);
            afterSpace = SeenWithin(1500);
          }
          string shown = "";
          OnUi(() => { shown = box.Text; });
          Press(0x1B); Thread.Sleep(100); Press(0x1B); Thread.Sleep(100); Press(0x1B);  // 取り消す
          Thread.Sleep(300);
          w.WriteLine(f[0] + "\t" + (paused ? 1 : 0) + "\t" + (afterSpace ? 1 : 0) + "\t" +
                      shown.Replace("\t", " ").Replace("\r", "").Replace("\n", ""));
          w.Flush();
        }
      }
    } catch (Exception e) {
      File.WriteAllText(args[3] + ".error.txt", e.ToString());
    }
    OnUi(Close);
  }

  [STAThread]
  static void Main(string[] a) {
    Application.EnableVisualStyles();
    Application.Run(new Probe(a));
  }
}
