// 画面に出す状態(XAML から結び付ける)。
using System.ComponentModel;
using System.Runtime.CompilerServices;
using WixToolset.BootstrapperApplicationApi;

namespace Asteria.Setup
{
    public sealed class SetupModel : INotifyPropertyChanged
    {
        private SetupPage _page = SetupPage.Loading;
        private string _version = "";
        private bool _withCuda;
        private bool _installed;
        private int _percent;
        private string _status = "準備しています";
        private string _errorText = "";
        private bool _restartRequired;
        private LaunchAction _action = LaunchAction.Install;

        public event PropertyChangedEventHandler PropertyChanged;

        public SetupPage Page { get => _page; set => Set(ref _page, value); }
        public string Version { get => _version; set => Set(ref _version, value); }
        public bool WithCuda { get => _withCuda; set { Set(ref _withCuda, value); Notify(nameof(DetectText)); } }
        public bool Installed { get => _installed; set => Set(ref _installed, value); }
        public int Percent { get => _percent; set => Set(ref _percent, value); }
        public string Status { get => _status; set => Set(ref _status, value); }
        // 扱っているファイルの名前(コピー中など。なければ空)。
        private string _detail = "";
        public string Detail { get => _detail; set => Set(ref _detail, value); }
        public string ErrorText { get => _errorText; set => Set(ref _errorText, value); }
        public bool RestartRequired { get => _restartRequired; set => Set(ref _restartRequired, value); }
        public LaunchAction Action { get => _action; set { Set(ref _action, value); Notify(nameof(SuccessTitle)); Notify(nameof(SuccessText)); Notify(nameof(ProgressTitle)); } }

        public string DetectText => "NVIDIA の GPU 向けの高速化も入れます";

        // インストール中に入れ替えて出すヒント(使い方の小さな発見。説明しすぎない)。
        public static readonly string[] Tips =
        {
            "ライブ変換は、設定でオンにできます",
            "Tab を押すと、AI が文の続きを予測します",
            "打った文章は、PC の外へ送りません",
            "覚えた言葉は、設定の「辞書」からいつでも消せます",
            "誤変換を見つけたら、設定画面から報告できます",
            "Windows キー + Space で、入力の切り替えができます",
            "予測を出し始める文字数は、設定の「予測」で変えられます",
        };

        public string ProgressTitle => Action == LaunchAction.Uninstall ? "アンインストールしています" : "インストールしています";

        public string SuccessTitle => Action switch
        {
            LaunchAction.Uninstall => "アンインストールが完了しました",
            LaunchAction.Repair => "修復が完了しました",
            _ => "インストールが完了しました",
        };

        public string SuccessText => Action == LaunchAction.Uninstall
            ? "ご利用ありがとうございました"
            : "Windows キー + Space で切り替えて使えます";

        private void Set<T>(ref T field, T value, [CallerMemberName] string name = null)
        {
            if (Equals(field, value)) return;
            field = value;
            Notify(name);
        }

        private void Notify(string name) => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
    }
}
