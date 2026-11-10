package dev.orchester.mobile;

import android.os.Bundle;
import android.graphics.Color;
import android.webkit.WebView;
import androidx.activity.OnBackPressedCallback;
import androidx.core.splashscreen.SplashScreen;
import com.getcapacitor.BridgeActivity;
import com.getcapacitor.WebViewListener;

public class MainActivity extends BridgeActivity {
    private boolean webReady;

    @Override public void onCreate(Bundle savedInstanceState) {
        SplashScreen splash = SplashScreen.installSplashScreen(this);
        splash.setKeepOnScreenCondition(() -> !webReady);
        bridgeBuilder.addWebViewListener(new WebViewListener() {
            @Override public void onPageLoaded(WebView webView) {
                webView.post(() -> webReady = true);
            }
        });
        registerPlugin(CredentialsPlugin.class);
        super.onCreate(savedInstanceState);
        getWindow().getDecorView().setBackgroundColor(Color.rgb(27, 17, 14));
        if (getBridge() != null) getBridge().getWebView().setBackgroundColor(Color.rgb(27, 17, 14));
        // A missing or failed WebView must still reveal the native error UI.
        getWindow().getDecorView().postDelayed(() -> webReady = true, 3500);
        getOnBackPressedDispatcher().addCallback(this, new OnBackPressedCallback(true) {
            @Override public void handleOnBackPressed() {
                WebView webView = getBridge() == null ? null : getBridge().getWebView();
                if (webView == null) {
                    leaveActivity();
                    return;
                }
                // Let Vue serialize this with pending header/picker traversals.
                // Calling goBack directly could queue a second traversal past root.
                webView.evaluateJavascript("window.orchesterHandleBack?.() ?? null", handled -> {
                    if (isFinishing() || isDestroyed() || "true".equals(handled)) return;
                    if ("null".equals(handled) && webView.canGoBack()) webView.goBack();
                    else leaveActivity();
                });
            }
            private void leaveActivity() {
                setEnabled(false);
                getOnBackPressedDispatcher().onBackPressed();
                setEnabled(true);
            }
        });
        splash.setOnExitAnimationListener(provider -> provider.getView().animate()
            .alpha(0f)
            .setDuration(160)
            .withEndAction(provider::remove)
            .start());
    }
}
