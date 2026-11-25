// 画面の出し分けとボタン。
using System;
using System.ComponentModel;
using System.Diagnostics;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media.Animation;
using System.Windows.Navigation;
using WixToolset.BootstrapperApplicationApi;

namespace Asteria.Setup
{
    public partial class SetupWindow : Window
    {
        private readonly AsteriaBA _ba;
        private readonly SetupModel _m;

        public SetupWindow(AsteriaBA ba) : this(ba, ba.Model) { }

        // プレビュー(Preview.cs)は ba なしで作る。
        internal SetupWindow(AsteriaBA ba, SetupModel model)
        {
            _ba = ba;
            _m = model;
            InitializeComponent();
            _m.PropertyChanged += OnModelChanged;
            Refresh();
        }

        private void OnModelChanged(object sender, PropertyChangedEventArgs e)
        {
            if (!Dispatcher.CheckAccess())
            {
                Dispatcher.Invoke(() => OnModelChanged(sender, e));
                return;
            }
            Refresh();
        }

        internal void Refresh()
        {
            VersionRun.Text = string.IsNullOrEmpty(_m.Version) ? "" : $"{_m.Version}";
            DetectText.Text = _m.DetectText;
            // NVIDIA の高速化は黙って入れる(利用者の依頼、2026-11-25: 表示しなくてよい)。
            DetectChip.Visibility = Visibility.Collapsed;
            ProgressTitle.Text = _m.ProgressTitle;
            StatusText.Text = _m.Status;
            PercentText.Text = $"{Math.Max(0, Math.Min(100, _m.Percent))}%";
            DetailText.Text = _m.Detail;
            UpdateRemaining();
            SuccessTitle.Text = _m.SuccessTitle;
            SuccessText.Text = _m.SuccessText;
            RestartText.Visibility = _m.RestartRequired ? Visibility.Visible : Visibility.Collapsed;
            ErrorText.Text = _m.ErrorText;
            SetBar(_m.Percent);

            Show(LoadingPage, _m.Page == SetupPage.Loading);
            Show(InstallPage, _m.Page == SetupPage.Install);
            Show(ModifyPage, _m.Page == SetupPage.Modify);
            Show(ProgressPage, _m.Page == SetupPage.Progress);
            Show(SuccessPage, _m.Page == SetupPage.Success);
            Show(FailurePage, _m.Page == SetupPage.Failure);
            CloseButton.Visibility = _m.Page == SetupPage.Progress ? Visibility.Hidden : Visibility.Visible;
            if (_m.Page == SetupPage.Progress && !_tips.IsEnabled) StartTips();
            if (_m.Page != SetupPage.Progress) _tips.Stop();
        }

        // 残り時間は、進行中の画面になってからの経過時間と % から見積もる(1 秒ごとに書き直す)。
        private DateTime? _progressStart;
        private readonly System.Windows.Threading.DispatcherTimer _clock =
            new System.Windows.Threading.DispatcherTimer { Interval = TimeSpan.FromSeconds(1) };
        internal TimeSpan? PreviewElapsed;  // プレビュー用

        private void UpdateRemaining()
        {
            if (_m.Page != SetupPage.Progress)
            {
                _clock.Stop();
                _progressStart = null;
                return;
            }
            if (_progressStart == null)
            {
                _progressStart = DateTime.UtcNow;
                _clock.Tick -= OnClock;
                _clock.Tick += OnClock;
                _clock.Start();
            }
            TimeSpan elapsed = PreviewElapsed ?? (DateTime.UtcNow - _progressStart.Value);
            RemainingText.Text = ProgressText.Remaining(elapsed, _m.Percent);
        }

        private void OnClock(object sender, EventArgs e) => UpdateRemaining();

        private readonly System.Windows.Threading.DispatcherTimer _tips =
            new System.Windows.Threading.DispatcherTimer { Interval = TimeSpan.FromSeconds(6) };
        private int _tip = new Random().Next(SetupModel.Tips.Length);

        // ヒントを 6 秒ごとに入れ替える(消えて、次がふわっと出る)。
        private void StartTips()
        {
            TipText.Text = SetupModel.Tips[_tip];
            _tips.Tick -= OnTip;
            _tips.Tick += OnTip;
            _tips.Start();
        }

        private void OnTip(object sender, EventArgs e)
        {
            var fadeOut = new DoubleAnimation(1, 0, TimeSpan.FromMilliseconds(200));
            fadeOut.Completed += (s, a) =>
            {
                _tip = (_tip + 1) % SetupModel.Tips.Length;
                TipText.Text = SetupModel.Tips[_tip];
                TipText.BeginAnimation(OpacityProperty, new DoubleAnimation(0, 1, TimeSpan.FromMilliseconds(260)));
            };
            TipText.BeginAnimation(OpacityProperty, fadeOut);
        }

        // 画面を切り替えるときは、ふわっと出す(0.18 秒)。
        private static void Show(UIElement page, bool visible)
        {
            if (visible == (page.Visibility == Visibility.Visible)) return;
            page.Visibility = visible ? Visibility.Visible : Visibility.Collapsed;
            if (visible)
            {
                page.BeginAnimation(OpacityProperty,
                    new DoubleAnimation(0, 1, TimeSpan.FromMilliseconds(180)) { EasingFunction = new QuadraticEase() });
            }
        }

        private void SetBar(int percent)
        {
            double total = ((FrameworkElement)Bar.Parent).ActualWidth;
            if (total <= 0) total = 360;
            double target = total * Math.Max(0, Math.Min(100, percent)) / 100.0;
            Bar.BeginAnimation(WidthProperty,
                new DoubleAnimation(target, TimeSpan.FromMilliseconds(250)) { EasingFunction = new QuadraticEase() });
        }

        private void OnDrag(object sender, MouseButtonEventArgs e)
        {
            if (e.ButtonState == MouseButtonState.Pressed) DragMove();
        }

        private void OnInstall(object sender, RoutedEventArgs e) => _ba.Start(LaunchAction.Install);
        private void OnRepair(object sender, RoutedEventArgs e) => _ba.Start(LaunchAction.Repair);
        private void OnUninstall(object sender, RoutedEventArgs e) => _ba.Start(LaunchAction.Uninstall);

        private void OnCancel(object sender, RoutedEventArgs e)
        {
            if (MessageBox.Show(this, "セットアップをやめますか?", "Asteria日本語入力",
                    MessageBoxButton.YesNo, MessageBoxImage.Question) == MessageBoxResult.Yes)
            {
                _ba.Cancel();
                CancelButton.IsEnabled = false;
            }
        }

        private void OnClose(object sender, RoutedEventArgs e) => Close();

        private void OnLicense(object sender, RequestNavigateEventArgs e)
        {
            Process.Start(new ProcessStartInfo(e.Uri.AbsoluteUri) { UseShellExecute = true });
            e.Handled = true;
        }
    }
}
