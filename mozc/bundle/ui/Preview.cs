// 開発用のプレビュー: 各画面を PNG に描く(インストールの仕組みは通らない)。
//   AsteriaSetupUI.exe --preview <出力フォルダー>
using System;
using System.IO;
using System.Windows;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using WixToolset.BootstrapperApplicationApi;

namespace Asteria.Setup
{
    internal static class Preview
    {
        public static void Render(string dir)
        {
            Directory.CreateDirectory(dir);
            var shots = new (string name, Action<SetupModel> set)[]
            {
                ("1-install-cuda", m => { m.WithCuda = true; m.Page = SetupPage.Install; }),
                ("2-install-plain", m => { m.WithCuda = false; m.Page = SetupPage.Install; }),
                ("3-progress", m => { m.WithCuda = true; m.Action = LaunchAction.Install; m.Status = "ファイルをコピーしています"; m.Detail = "genmai-310M-q8_0.gguf"; m.Percent = 62; m.Page = SetupPage.Progress; }),
                ("4-success", m => { m.Action = LaunchAction.Install; m.Page = SetupPage.Success; }),
                ("5-modify", m => { m.Installed = true; m.Page = SetupPage.Modify; }),
                ("6-failure", m => { m.ErrorText = "ほかのインストールが動いています。終わってからもう一度試してください。"; m.Page = SetupPage.Failure; }),
            };
            foreach (var (name, set) in shots)
            {
                var model = new SetupModel { Version = "5.5.0" };
                set(model);
                var w = new SetupWindow(null, model) { ShowActivated = false, Left = -10000, Top = -10000,
                                                      PreviewElapsed = TimeSpan.FromSeconds(95) };
                model.Percent = model.Percent;  // 残り時間をプレビューの経過時間で描き直す
                w.Refresh();
                w.Show();
                w.UpdateLayout();
                // 切り替えのアニメーションを終えた状態で描く。
                foreach (UIElement page in new UIElement[] { w.InstallPage, w.ProgressPage, w.SuccessPage, w.ModifyPage, w.FailurePage, w.LoadingPage })
                {
                    page.BeginAnimation(UIElement.OpacityProperty, null);
                    page.Opacity = 1;
                }
                w.Bar.BeginAnimation(FrameworkElement.WidthProperty, null);
                w.Bar.Width = ((FrameworkElement)w.Bar.Parent).ActualWidth * model.Percent / 100.0;
                w.UpdateLayout();
                // 影の余白(Margin)ごと、窓全体を描く。
                var content = (FrameworkElement)w.Content;
                double width = content.ActualWidth + content.Margin.Left + content.Margin.Right;
                double height = content.ActualHeight + content.Margin.Top + content.Margin.Bottom;
                double scale = 2.0;  // 200% の画面と同じ細かさで描く
                var bmp = new RenderTargetBitmap((int)(width * scale), (int)(height * scale),
                                                 96 * scale, 96 * scale, PixelFormats.Pbgra32);
                bmp.Render(w);
                var enc = new PngBitmapEncoder();
                enc.Frames.Add(BitmapFrame.Create(bmp));
                using (var f = File.Create(Path.Combine(dir, name + ".png"))) enc.Save(f);
                w.Close();
            }
        }
    }
}
