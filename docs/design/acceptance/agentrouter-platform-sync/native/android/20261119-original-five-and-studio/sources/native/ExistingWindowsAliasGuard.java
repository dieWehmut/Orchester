import java.security.KeyStore;

public final class ExistingWindowsAliasGuard {
    public static void main(String[] args) {
        try {
            if (args.length != 1) throw new SecurityException("expected-native-app-uid-required");
            int expectedUid = Integer.parseInt(args[0]);
            int currentUid = android.os.Process.myUid();
            if (expectedUid < 10000 || currentUid != expectedUid) {
                throw new SecurityException("unexpected-native-app-uid");
            }
            if (java.security.Security.getProvider("AndroidKeyStore") == null) {
                Class.forName("android.security.keystore2.AndroidKeyStoreProvider")
                    .getMethod("install").invoke(null);
            }
            KeyStore store = KeyStore.getInstance("AndroidKeyStore");
            store.load(null);
            boolean exists = store.containsAlias("orchester.remote.credentials.v1");
            System.out.println("{\"appUid\":" + currentUid
                + ",\"expectedNativeAppUidMatched\":true,\"existingAliasPresent\":" + exists
                + ",\"keyGenerationCalls\":0,\"credentialReads\":0}");
            System.exit(exists ? 0 : 2);
        } catch (Throwable error) {
            System.out.println("{\"existingAliasPresent\":null,\"keyGenerationCalls\":0,"
                + "\"credentialReads\":0,\"errorType\":\""
                + error.getClass().getSimpleName() + "\"}");
            System.exit(1);
        }
    }
}
