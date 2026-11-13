import java.security.KeyStore;
public final class ExistingAliasGuardWithProvider {
 public static void main(String[] args) {
  try {
   if (android.os.Process.myUid() != 10216) throw new SecurityException("unexpected-app-uid");
   if (java.security.Security.getProvider("AndroidKeyStore") == null) {
    Class.forName("android.security.keystore2.AndroidKeyStoreProvider").getMethod("install").invoke(null);
   }
   KeyStore store = KeyStore.getInstance("AndroidKeyStore");
   store.load(null);
   boolean exists = store.containsAlias("orchester.remote.credentials.v1");
   System.out.println("{\"appUid\":10216,\"existingAliasPresent\":" + exists + ",\"keyGenerationCalls\":0,\"credentialReads\":0}");
   System.exit(exists ? 0 : 2);
  } catch (Throwable error) {
   System.out.println("{\"existingAliasPresent\":null,\"keyGenerationCalls\":0,\"credentialReads\":0,\"errorType\":\"" + error.getClass().getSimpleName() + "\"}");
   System.exit(1);
  }
 }
}
