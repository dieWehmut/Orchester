package dev.orchester.mobile;

import static org.junit.Assert.*;

import android.content.Context;
import android.content.SharedPreferences;
import android.os.SystemClock;
import androidx.test.core.app.ActivityScenario;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.FutureTask;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import org.json.JSONObject;
import org.junit.Test;
import org.junit.runner.RunWith;

/** Exercises the installed WebView bridge, Android Keystore and native HTTP stack. */
@RunWith(AndroidJUnit4.class)
public class NativeBridgeTest {
    private static final String KEY = "native-qa-device";
    private static final String OTHER_KEY = "native-qa-other-device";
    private static final String TOKEN = "native-qa-only-token-not-a-real-credential";

    @Test
    public void themeChangesAllSurfacesAndSurvivesActivityRecreation() throws Exception {
        try (ActivityScenario<MainActivity> app = ActivityScenario.launch(MainActivity.class)) {
            awaitBridge(app);
            String previous = evaluate(app, "localStorage.getItem('orchester.theme')");
            try {
                evaluate(app, "document.querySelectorAll('nav button')[3].click()");
                awaitThemeButtons(app);
                evaluate(app, "document.querySelectorAll('.theme-option')[2].click()");
                assertEquals("\"sea\"", evaluate(app, "document.documentElement.dataset.theme"));
                assertEquals("\"#0e181a\"", evaluate(app, "getComputedStyle(document.documentElement).getPropertyValue('--bg').trim()"));
                assertEquals("\"sea\"", evaluate(app, "localStorage.getItem('orchester.theme')"));
                app.recreate();
                awaitBridge(app);
                assertEquals("\"sea\"", evaluate(app, "document.documentElement.dataset.theme"));
            } finally {
                evaluate(app, previous.equals("null") ? "localStorage.removeItem('orchester.theme')" : "localStorage.setItem('orchester.theme'," + previous + ")");
            }
        }
    }

    private void awaitThemeButtons(ActivityScenario<MainActivity> app) throws Exception {
        long deadline = SystemClock.elapsedRealtime() + 10000;
        while (SystemClock.elapsedRealtime() < deadline) {
            if (evaluate(app, "document.querySelectorAll('.theme-option').length").equals("6")) return;
            // Navigate using the rendered theme entry, independent of its palette label.
            evaluate(app, "Array.from(document.querySelectorAll('.list-row')).find(button => button.textContent.includes('选择你的配色'))?.click()");
            SystemClock.sleep(100);
        }
        fail("Theme page did not render six choices");
    }

    @Test
    public void credentialsSurviveRecreationAndAreEncryptedAtRest() throws Exception {
        Context context = InstrumentationRegistry.getInstrumentation().getTargetContext();
        assertEquals("dev.orchester.mobile", context.getPackageName());
        SharedPreferences preferences = context.getSharedPreferences("remote_credentials", Context.MODE_PRIVATE);
        try (ActivityScenario<MainActivity> app = ActivityScenario.launch(MainActivity.class)) {
            awaitBridge(app);
            assertEquals("true", evaluate(app, "document.querySelector('nav[aria-label=主导航]') !== null"));
            call(app, "Capacitor.Plugins.Credentials.set({key:" + JSONObject.quote(KEY) + ",value:" + JSONObject.quote(TOKEN) + "})");
            String encrypted = preferences.getString(KEY, null);
            assertNotNull(encrypted);
            assertFalse(encrypted.contains(TOKEN));
            assertTrue("Stored record has nonce and ciphertext", encrypted.contains(":"));

            app.recreate();
            awaitBridge(app);
            assertEquals(TOKEN, call(app, "Capacitor.Plugins.Credentials.get({key:" + JSONObject.quote(KEY) + "})").getString("value"));

            // AES-GCM associated data must bind a credential to its device key.
            assertTrue(preferences.edit().putString(OTHER_KEY, encrypted).commit());
            JSONObject result = call(app, "Capacitor.Plugins.Credentials.get({key:" + JSONObject.quote(OTHER_KEY) + "})"
                + ".then(() => ({rejected:false}), () => ({rejected:true}))");
            assertTrue("Moving ciphertext to another device must fail", result.getBoolean("rejected"));

            call(app, "Capacitor.Plugins.Credentials.remove({key:" + JSONObject.quote(KEY) + "})");
            assertTrue(call(app, "Capacitor.Plugins.Credentials.get({key:" + JSONObject.quote(KEY) + "})").isNull("value"));
            assertFalse(preferences.contains(KEY));
        } finally {
            assertTrue(preferences.edit().remove(KEY).remove(OTHER_KEY).commit());
        }
    }

    @Test
    public void nativeHttpCanReachCleartextServerWithBearerHeaderWithoutCors() throws Exception {
        try (ServerSocket server = new ServerSocket(0, 1, InetAddress.getByName("127.0.0.1"));
             ActivityScenario<MainActivity> app = ActivityScenario.launch(MainActivity.class)) {
            server.setSoTimeout(30000);
            FutureTask<String> request = new FutureTask<>(() -> {
                try (Socket socket = server.accept()) {
                    socket.setSoTimeout(15000);
                    BufferedReader reader = new BufferedReader(new InputStreamReader(socket.getInputStream(), StandardCharsets.UTF_8));
                    StringBuilder headers = new StringBuilder();
                    for (String line; (line = reader.readLine()) != null && !line.isEmpty();) {
                        headers.append(line).append('\n');
                    }
                    byte[] body = "{\"device\":\"native-qa\"}".getBytes(StandardCharsets.UTF_8);
                    String response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: "
                        + body.length + "\r\nConnection: close\r\n\r\n";
                    socket.getOutputStream().write(response.getBytes(StandardCharsets.UTF_8));
                    socket.getOutputStream().write(body);
                    socket.getOutputStream().flush();
                    return headers.toString();
                }
            });
            Thread responder = new Thread(request, "native-qa-http");
            responder.setDaemon(true);
            responder.start();
            awaitBridge(app);
            JSONObject response = call(app, "Capacitor.Plugins.CapacitorHttp.request({method:'GET',url:"
                + JSONObject.quote("http://127.0.0.1:" + server.getLocalPort() + "/remote/v1/info")
                + ",headers:{Authorization:'Bearer native-qa'},connectTimeout:10000,readTimeout:10000})");
            assertEquals(200, response.getInt("status"));
            assertEquals("native-qa", response.getJSONObject("data").getString("device"));
            String headers = request.get(15, TimeUnit.SECONDS);
            assertTrue(headers.startsWith("GET /remote/v1/info HTTP/1.1\n"));
            assertTrue(headers.toLowerCase(java.util.Locale.ROOT).contains("authorization: bearer native-qa\n"));
        }
    }

    private void awaitBridge(ActivityScenario<MainActivity> app) throws Exception {
        long deadline = SystemClock.elapsedRealtime() + 30000;
        while (SystemClock.elapsedRealtime() < deadline) {
            if (evaluate(app, "Boolean(window.Capacitor?.Plugins?.Credentials && window.Capacitor?.Plugins?.CapacitorHttp && document.querySelector('#app')?.textContent)").equals("true")) return;
            SystemClock.sleep(100);
        }
        fail("Packaged app did not register its native bridge within 30 seconds");
    }

    private JSONObject call(ActivityScenario<MainActivity> app, String expression) throws Exception {
        evaluate(app, "window.__nativeQaResult = undefined; Promise.resolve().then(() => (" + expression
            + ")).then(value => {window.__nativeQaResult = {ok:true,value:value ?? {}}}, error => {window.__nativeQaResult = {ok:false,error:String(error)}}); void 0");
        long deadline = SystemClock.elapsedRealtime() + 30000;
        while (SystemClock.elapsedRealtime() < deadline) {
            String result = evaluate(app, "window.__nativeQaResult ?? null");
            if (!result.equals("null")) {
                JSONObject parsed = new JSONObject(result);
                assertTrue("Native bridge rejected: " + parsed.optString("error"), parsed.getBoolean("ok"));
                return parsed.getJSONObject("value");
            }
            SystemClock.sleep(100);
        }
        throw new AssertionError("Native bridge call timed out");
    }

    private String evaluate(ActivityScenario<MainActivity> app, String script) throws Exception {
        CountDownLatch completed = new CountDownLatch(1);
        AtomicReference<String> result = new AtomicReference<>();
        app.onActivity(activity -> activity.getBridge().getWebView().evaluateJavascript(script, value -> {
            result.set(value);
            completed.countDown();
        }));
        assertTrue("WebView JavaScript timed out", completed.await(10, TimeUnit.SECONDS));
        return result.get();
    }
}
