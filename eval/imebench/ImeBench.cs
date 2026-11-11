// 実際の IME(TSF のテキストサービス)で変換の正確さを測る(Google 日本語入力・Microsoft IME・Kotori を
// 同じ条件で比べる。eval/imebench/README.md)。
//
// テキストボックスの窓を前に出し、指定の IME をこのプロセスで有効にして、読みのローマ字をキーとして送り、
// Space(変換)→ Enter(確定)を押して、確定した文字を読む。前の文は渡さない(どの IME にも同じ条件)。
//
// 使い方: ImeBench.exe <テキストサービスの CLSID> <プロファイルの GUID> <入力.tsv> <出力.tsv> [Space の後の待ち ms]
//   入力.tsv: 1 行に「番号<TAB>ローマ字[<TAB>前文[<TAB>nospace]]」。出力.tsv: 1 行に「番号<TAB>確定した文字」。
//   前文があれば入力欄に置いてカーソルを末尾にしてから打ち、確定した文字は前文を除いて書く(前文を読む IME の比較)。
//   nospace なら Space を押さずに確定する(句読点で自動変換する設定で、。？！ で終わる問。2026-10-23)。
// 計測の間、この PC のキーボード入力を使う(ほかの操作をしない)。
using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Forms;

[ComImport, Guid("71C6E74C-0F28-11D8-A82A-00065B84435C"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface ITfInputProcessorProfileMgr {
  [PreserveSig] int ActivateProfile(uint dwProfileType, ushort langid, ref Guid clsid, ref Guid guidProfile,
                                    IntPtr hkl, uint dwFlags);
  [PreserveSig] int DeactivateProfile(uint dwProfileType, ushort langid, ref Guid clsid, ref Guid guidProfile,
                                      IntPtr hkl, uint dwFlags);
  [PreserveSig] int GetProfile(uint dwProfileType, ushort langid, ref Guid clsid, ref Guid guidProfile,
                               IntPtr hkl, out TfInputProcessorProfile profile);
  [PreserveSig] int EnumProfiles(ushort langid, out IntPtr enumerator);
  [PreserveSig] int ReleaseInputProcessor(ref Guid clsid, uint dwFlags);
  [PreserveSig] int RegisterProfile(ref Guid clsid, ushort langid, ref Guid guidProfile, IntPtr desc, uint cch,
                                    IntPtr iconFile, uint cchFile, uint iconIndex, IntPtr hklSubstitute,
                                    uint dwPreferredLayout, int enabledByDefault, uint dwFlags);
  [PreserveSig] int UnregisterProfile(ref Guid clsid, ushort langid, ref Guid guidProfile, uint dwFlags);
  [PreserveSig] int GetActiveProfile(ref Guid catid, out TfInputProcessorProfile profile);
}

[StructLayout(LayoutKind.Sequential)]
struct TfInputProcessorProfile {
  public uint dwProfileType; public ushort langid; public Guid clsid; public Guid guidProfile; public Guid catid;
  public IntPtr hklSubstitute; public uint dwCaps; public IntPtr hkl; public uint dwFlags;
}

static class Native {
  [StructLayout(LayoutKind.Sequential)]
  public struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Sequential)]
  public struct INPUT { public uint type; public KEYBDINPUT ki; public long pad; }
  [DllImport("user32.dll", SetLastError = true)] public static extern uint SendInput(uint n, INPUT[] inputs, int size);
  [DllImport("user32.dll")] public static extern short VkKeyScanW(char ch);
  [DllImport("user32.dll")] public static extern uint MapVirtualKeyW(uint code, uint type);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
  [DllImport("imm32.dll")] public static extern IntPtr ImmGetContext(IntPtr h);
  [DllImport("imm32.dll")] public static extern bool ImmReleaseContext(IntPtr h, IntPtr c);
  [DllImport("imm32.dll")] public static extern bool ImmSetOpenStatus(IntPtr c, bool open);
  [DllImport("imm32.dll")] public static extern bool ImmSetConversionStatus(IntPtr c, uint conv, uint sentence);
}

class Bench : Form {
  readonly TextBox box = new TextBox();
  // 問ごとにフォーカスを移して戻し、IME が前の問の確定を文脈として持ち越さないようにする。
  readonly TextBox other = new TextBox();
  readonly string[] args;
  public Bench(string[] a) {
    args = a;
    Text = "ImeBench";
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

  static void Press(ushort vk, bool shift = false) {
    if (shift) Key(0x10, false);
    Key(vk, false);
    Key(vk, true);
    if (shift) Key(0x10, true);
  }

  static void TypeKeys(string romaji, int keyMs) {
    foreach (char ch in romaji) {
      short r = Native.VkKeyScanW(ch);
      if (r == -1) continue;
      Press((ushort)(r & 0xff), (r & 0x100) != 0);
      Thread.Sleep(keyMs);
    }
  }

  void OnUi(Action a) { Invoke(a); }

  // この窓を前面にする。別の窓が前面のままキーを送ると、その窓(チャットの入力欄など)に打ち込んで
  // しまう(2026-11-11 に起きた)。Alt を押して離すと前面に出せる。出せなければ止める。
  void EnsureForeground() {
    for (int attempt = 0; attempt < 5; attempt++) {
      IntPtr mine = IntPtr.Zero;
      OnUi(() => { mine = Handle; });
      if (Native.GetForegroundWindow() == mine) return;
      Native.keybd_event(0x12, 0, 0, IntPtr.Zero);
      OnUi(() => { Native.SetForegroundWindow(Handle); Activate(); box.Focus(); });
      Native.keybd_event(0x12, 0, 2, IntPtr.Zero);
      Thread.Sleep(300);
    }
    throw new Exception("ImeBench の窓を前面にできない(ほかの窓にキーを送らないよう止めた)");
  }

  void Run() {
    try {
      Guid clsid = new Guid(args[0]), profile = new Guid(args[1]);
      int afterSpace = args.Length > 4 ? int.Parse(args[4]) : 1200;
      OnUi(() => {
        var mgr = (ITfInputProcessorProfileMgr)Activator.CreateInstance(
            Type.GetTypeFromCLSID(new Guid("33C53A50-F456-4884-B049-85FD643ECFED")));
        // TF_PROFILETYPE_INPUTPROCESSOR、日本語、このプロセスだけ・今の入力言語に関係なく
        // 前に使った IME のままになることがある(2026-11-11、Google の計測が Asteria で変換されていた)。
        // 有効になった IME を確かめ、違えば何回かやり直し、それでも違えば止める。
        Guid keyboard = new Guid("34745C63-B2F0-4784-8B67-5E12C8701A31");  // GUID_TFCAT_TIP_KEYBOARD
        for (int attempt = 0; ; attempt++) {
          int hr = mgr.ActivateProfile(1, 0x0411, ref clsid, ref profile, IntPtr.Zero, 0x10000000 | 0x4 | 0x1);
          if (hr != 0) throw new Exception("ActivateProfile 0x" + hr.ToString("x"));
          TfInputProcessorProfile active;
          if (mgr.GetActiveProfile(ref keyboard, out active) == 0 && active.clsid == clsid &&
              active.guidProfile == profile) break;
          if (attempt >= 4) throw new Exception("指定の IME が有効にならない(有効なのは " + active.clsid + ")");
          Thread.Sleep(500);
        }
      });
      Thread.Sleep(1500);
      using (var w = new StreamWriter(args[3], false, new UTF8Encoding(false))) {
        foreach (string line in File.ReadAllLines(args[2], Encoding.UTF8)) {
          string[] f = line.Split('\t');
          if (f.Length < 2) continue;
          // 「#wait<TAB>ファイル」はファイルができるまで待つ。開いたままのアプリで、IME の更新の
          // 前後に入力が続くかを確かめる(docs/adr/0072)。
          if (f[0] == "#wait") {
            while (!File.Exists(f[1])) Thread.Sleep(500);
            continue;
          }
          OnUi(() => { other.Focus(); });
          Thread.Sleep(150);
          string context = f.Length > 2 ? f[2] : "";
          bool noSpace = f.Length > 3 && f[3] == "nospace";
          OnUi(() => {
            box.Clear();
            if (context.Length > 0) {
              box.Text = context;
              box.SelectionStart = context.Length;
            }
            Native.SetForegroundWindow(Handle);
            box.Focus();
            IntPtr c = Native.ImmGetContext(box.Handle);
            Native.ImmSetOpenStatus(c, true);
            Native.ImmSetConversionStatus(c, 0x1 | 0x8, 0x8);  // ひらがな・全角、一般
            Native.ImmReleaseContext(box.Handle, c);
          });
          Thread.Sleep(150);
          EnsureForeground();
          TypeKeys(f[1], 12);
          Thread.Sleep(250);
          // Asteria: 第 6 引数が live なら、ライブ変換(docs/adr/0077)を確かめる。Space を押さずに待ち、
          // Enter の前の未確定の表示を記録してから Enter で確定する。
          bool live = args.Length > 5 && args[5] == "live";
          string shown = "";
          EnsureForeground();
          if (!live && !noSpace) Press(0x20);  // Space(変換)
          Thread.Sleep(afterSpace);
          if (live) OnUi(() => { shown = box.Text; });
          EnsureForeground();
          Press(0x0D);  // Enter(確定)
          Thread.Sleep(250);
          string got = "";
          OnUi(() => { got = box.Text; });
          if (context.Length > 0 && got.StartsWith(context)) got = got.Substring(context.Length);
          w.WriteLine(f[0] + "\t" + got.Replace("\t", " ").Replace("\r", "").Replace("\n", "") +
                      (live ? "\t" + shown.Replace("\t", " ").Replace("\r", "").Replace("\n", "") : ""));
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
    Application.Run(new Bench(a));
  }
}
