// Asteria-Setup.exe の画面の本体(docs/adr/0091)。Burn から起こされ、見分け(Detect)→ 計画(Plan)→ 実行(Apply)を
// 進める。/quiet・/passive のときは窓を出さずに同じ流れで進める。
using System;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Threading;
using WixToolset.BootstrapperApplicationApi;

namespace Asteria.Setup
{
    internal static class Program
    {
        [STAThread]
        private static int Main(string[] args)
        {
            // 開発用: --preview <フォルダー> で、各画面を PNG に描いて終わる(Burn なし、何も入れない)。
            if (args.Length == 2 && args[0] == "--preview")
            {
                Preview.Render(args[1]);
                return 0;
            }
            var app = new AsteriaBA();
            ManagedBootstrapperApplication.Run(app);
            return 0;
        }
    }

    public enum SetupPage { Loading, Install, Modify, Progress, Success, Failure }

    public sealed class AsteriaBA : BootstrapperApplication
    {
        // NVIDIA の高速化を入れる条件(AsteriaSetup.wxs の InstallCondition と同じ)。
        public const string CudaCondition = "NvCudaVersion >= v32.0.15.8000";

        private IEngine _engine;
        private IBootstrapperCommand _command;
        private Dispatcher _dispatcher;
        private SetupWindow _window;
        private LaunchAction _planned = LaunchAction.Install;
        private bool _installed;
        private bool _canceled;
        private int _exitCode;

        public SetupModel Model { get; } = new SetupModel();

        private bool ShowUi => _command.Display == Display.Full || _command.Display == Display.Passive;
        private bool Interactive => _command.Display == Display.Full;

        protected override void OnCreate(CreateEventArgs args)
        {
            base.OnCreate(args);
            _engine = args.Engine;
            _command = args.Command;
        }

        protected override void Run()
        {
            _dispatcher = Dispatcher.CurrentDispatcher;
            Model.Version = _engine.GetVariableVersion("WixBundleVersion");
            if (ShowUi)
            {
                _window = new SetupWindow(this);
                _window.Closed += (s, e) => _dispatcher.InvokeShutdown();
                _window.Show();
            }
            _engine.Detect();
            Dispatcher.Run();
            _engine.Quit(_exitCode);
        }

        // 画面のボタンから呼ぶ。
        public void Start(LaunchAction action)
        {
            _planned = action;
            Model.Action = action;
            Model.Page = SetupPage.Progress;
            _engine.Plan(action);
        }

        public void Cancel() => _canceled = true;

        public void Close() => _dispatcher.InvokeShutdown();

        protected override void OnDetectPackageComplete(DetectPackageCompleteEventArgs args)
        {
            if (args.PackageId == "Asteria" && args.State == PackageState.Present) _installed = true;
            base.OnDetectPackageComplete(args);
        }

        protected override void OnDetectComplete(DetectCompleteEventArgs args)
        {
            base.OnDetectComplete(args);
            bool cuda = _engine.EvaluateCondition(CudaCondition);
            _dispatcher.Invoke(() =>
            {
                Model.WithCuda = cuda;
                Model.Installed = _installed;
                if (args.Status < 0)
                {
                    Fail(args.Status, null);
                    return;
                }
                // 対話でないとき、またはアンインストールを頼まれたときは、そのまま進める。
                if (!Interactive || _command.Action == LaunchAction.Uninstall)
                {
                    Start(_command.Action == LaunchAction.Unknown ? LaunchAction.Install : _command.Action);
                    return;
                }
                Model.Page = _installed ? SetupPage.Modify : SetupPage.Install;
            });
        }

        protected override void OnPlanComplete(PlanCompleteEventArgs args)
        {
            base.OnPlanComplete(args);
            if (args.Status < 0)
            {
                _dispatcher.Invoke(() => Fail(args.Status, null));
                return;
            }
            IntPtr hwnd = IntPtr.Zero;
            _dispatcher.Invoke(() =>
            {
                if (_window != null) hwnd = new WindowInteropHelper(_window).Handle;
            });
            _engine.Apply(hwnd);
        }

        protected override void OnExecutePackageBegin(ExecutePackageBeginEventArgs args)
        {
            base.OnExecutePackageBegin(args);
            // 高速化(AsteriaCuda)も本体と同じ言い方にする(NVIDIA の話は出さない。利用者の依頼、2026-11-25)。
            string text = _planned == LaunchAction.Uninstall ? "アンインストールしています" : "インストールしています";
            _dispatcher.Invoke(() => Model.Status = text);
        }

        // --- いま何をしているか(段階と、扱っているファイル) ---

        protected override void OnCacheAcquireBegin(CacheAcquireBeginEventArgs args)
        {
            base.OnCacheAcquireBegin(args);
            Phase("インストーラーの中身を取り出しています", "");
        }

        protected override void OnCacheVerifyBegin(CacheVerifyBeginEventArgs args)
        {
            base.OnCacheVerifyBegin(args);
            Phase("取り出したファイルを確かめています", "");
        }

        protected override void OnExecuteMsiMessage(ExecuteMsiMessageEventArgs args)
        {
            base.OnExecuteMsiMessage(args);
            if (args.MessageType == InstallMessage.ActionStart)
            {
                string phase = ProgressText.ForAction(args.Message, _planned == LaunchAction.Uninstall);
                if (phase != null) Phase(phase, "");
            }
            else if (args.MessageType == InstallMessage.ActionData)
            {
                string file = ProgressText.FileIn(args.Message);
                if (file != null) _dispatcher.BeginInvoke(new Action(() => Model.Detail = file));
            }
        }

        private void Phase(string status, string detail)
        {
            _dispatcher.BeginInvoke(new Action(() =>
            {
                Model.Status = status;
                Model.Detail = detail;
            }));
        }

        protected override void OnProgress(ProgressEventArgs args)
        {
            base.OnProgress(args);
            _dispatcher.Invoke(() => Model.Percent = args.OverallPercentage);
            if (_canceled) args.Cancel = true;
        }

        protected override void OnExecuteProgress(ExecuteProgressEventArgs args)
        {
            base.OnExecuteProgress(args);
            if (_canceled) args.Cancel = true;
        }

        protected override void OnError(ErrorEventArgs args)
        {
            base.OnError(args);
            if (!string.IsNullOrEmpty(args.ErrorMessage))
            {
                _dispatcher.Invoke(() => Model.ErrorText = args.ErrorMessage);
            }
        }

        protected override void OnApplyComplete(ApplyCompleteEventArgs args)
        {
            base.OnApplyComplete(args);
            _dispatcher.Invoke(() =>
            {
                Model.RestartRequired = args.Restart == ApplyRestart.RestartRequired;
                if (args.Status < 0)
                {
                    Fail(args.Status, _canceled ? "セットアップをやめました。" : null);
                    return;
                }
                _exitCode = 0;
                if (_planned == LaunchAction.Install || _planned == LaunchAction.Repair || _planned == LaunchAction.Modify)
                {
                    _engine.Log(LogLevel.Standard, "Asteria input list: " + InputList.Ensure());
                }
                else if (_planned == LaunchAction.Uninstall)
                {
                    _engine.Log(LogLevel.Standard, "Asteria input list: " + InputList.Remove());
                }
                Model.Percent = 100;
                Model.Page = SetupPage.Success;
                // 対話でないときは、終わったら閉じる。
                if (!Interactive) _dispatcher.InvokeShutdown();
            });
        }

        private void Fail(int status, string message)
        {
            _exitCode = status;
            if (message != null) Model.ErrorText = message;
            Model.Page = SetupPage.Failure;
            if (!Interactive) _dispatcher.InvokeShutdown();
        }
    }
}
