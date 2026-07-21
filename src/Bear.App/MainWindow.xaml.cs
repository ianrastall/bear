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
    /// Record a game abandoned mid-play, so quitting is not a way to lose one.
    ///
    /// Nothing is asked here. Every finished game is already on disk, so there is no
    /// decision left to put in front of someone who is trying to close a window.
    /// </summary>
    private void OnClosing(
        Microsoft.UI.Windowing.AppWindow sender,
        Microsoft.UI.Windowing.AppWindowClosingEventArgs args)
    {
        if (RootFrame.Content is MainPage page)
            page.FlushGameLog();
    }
}
