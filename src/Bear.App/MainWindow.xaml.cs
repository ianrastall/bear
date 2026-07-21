using Microsoft.UI.Xaml;

// To learn more about WinUI, the WinUI project structure,
// and more about our project templates, see: http://aka.ms/winui-project-info.

namespace Bear.App;

/// <summary>
/// The application window. This hosts a Frame that displays pages. Add your
/// UI and logic to MainPage.xaml / MainPage.xaml.cs instead of here so you
/// can use Page features such as navigation events and the Loaded lifecycle.
/// </summary>
public sealed partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();

        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);

        AppWindow.SetIcon("Assets/AppIcon.ico");

        // The board is square and is the whole app, so the window starts square-ish:
        // just enough extra height for the two clock strips.
        AppWindow.Resize(new Windows.Graphics.SizeInt32(780, 880));

        // Navigate the root frame to the main page on startup.
        RootFrame.Navigate(typeof(MainPage));

        AppWindow.Closing += OnClosing;
    }

    /// <summary>
    /// Guards against re-entry: the close we perform ourselves after the prompt must not
    /// raise the prompt again.
    /// </summary>
    private bool _closeConfirmed;

    /// <summary>
    /// Offer to export the session's games before quitting.
    ///
    /// <see cref="Microsoft.UI.Windowing.AppWindow.Closing"/> cannot be awaited, so the
    /// close is cancelled, the dialog is shown, and the window closes itself afterwards if
    /// the answer allows it.
    /// </summary>
    private async void OnClosing(
        Microsoft.UI.Windowing.AppWindow sender,
        Microsoft.UI.Windowing.AppWindowClosingEventArgs args)
    {
        if (_closeConfirmed || RootFrame.Content is not MainPage page)
            return;

        args.Cancel = true;

        if (await page.ConfirmExitAsync())
        {
            _closeConfirmed = true;
            Close();
        }
    }
}
