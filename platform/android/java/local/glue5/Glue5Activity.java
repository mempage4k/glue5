package local.glue5;

import android.app.AlertDialog;
import android.content.ActivityNotFoundException;
import android.content.ComponentName;
import android.content.ClipData;
import android.content.Context;
import android.content.ClipboardManager;
import android.content.ContentResolver;
import android.content.Intent;
import android.content.ServiceConnection;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.net.Uri;
import android.net.wifi.WifiManager;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.IBinder;
import android.os.Environment;
import android.os.Looper;
import android.os.ParcelFileDescriptor;
import android.os.VibrationAttributes;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.os.VibratorManager;
import android.provider.DocumentsContract;
import android.provider.Settings;
import android.system.ErrnoException;
import android.system.Os;
import android.util.Log;
import android.view.WindowManager;
import android.widget.Toast;

import androidx.core.content.FileProvider;
import org.libsdl.app.SDLActivity;
import org.libsdl.app.SDLSurface;
import rikka.shizuku.Shizuku;

import java.io.BufferedInputStream;
import java.io.BufferedOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;

/** The app's activity: SDL's, with our library (libglue5.so) as the main one. */
public class Glue5Activity extends SDLActivity {
    private static final int IMPORT_TREE = 4105;
    private static final int GAME_TREE = 4106;
    private static final int ALL_FILES = 4107;
    private static final int SHIZUKU_LOGCAT_PERMISSION = 0x4755;
    private static final int LOGCAT_SNAPSHOT = 0;
    private static final int LOGCAT_LIVE = 1;
    private static final int MAX_CLIPBOARD_BYTES = 256 * 1024;
    private static final String IMPORT_PREFS = "glue5-import";
    private static final String LOGCAT_PREFS = "glue5-logcat";
    private static final String PENDING_CRASH_LOGCAT = "pending-crash-logcat";
    private static final String PENDING_CRASH_PID = "pending-crash-pid";
    private static final String SESSION_ACTIVE = "session-active";
    private static final String SESSION_PID = "session-pid";
    private static final String PENDING_KIND = "kind";
    private static final String PENDING_URI = "uri";
    /** The kind that asks for the game's own folder, rather than files to copy in. */
    private static final String GAME_KIND = "game";
    /** Where Rust reads the folder chosen for the game, or why there is none. */
    private static final String GAME_PATH = "game-path";
    private static final String GAME_ERROR = "game-error";
    private static final String GAME_FILES_HELP = "Use Find files to choose the game folder wherever it is "
            + "(recommended). Advanced: from a computer, run adb push game "
            + "/sdcard/Android/data/local.glue5/files/.";
    private static final AtomicBoolean COPYING = new AtomicBoolean();
    private static final AtomicBoolean LOGCAT_CAPTURE = new AtomicBoolean();
    private static final ExecutorService IMPORTS = Executors.newSingleThreadExecutor();
    private static final ExecutorService LOGCAT_WORKER = Executors.newSingleThreadExecutor();
    private static final Shizuku.UserServiceArgs LOGCAT_SERVICE_ARGS =
            new Shizuku.UserServiceArgs(new ComponentName("local.glue5", ShizukuLogcatService.class.getName()))
                    .daemon(false)
                    .processNameSuffix("logcat")
                    .version(3);
    private static volatile Glue5Activity active;
    private static final Object LAN_LOCK_MONITOR = new Object();
    private static WifiManager.MulticastLock lanMulticastLock;
    private static boolean lanDiscoveryRequested;
    private static volatile boolean phoneRumbleActive;
    private static final AtomicBoolean HAPTIC_SETTING_WARNING = new AtomicBoolean();
    private volatile int requestedLogcatMode = LOGCAT_SNAPSHOT;
    private volatile int snapshotLogcatPid;
    private volatile int requestedLogcatPid;
    private volatile ILogcatCaptureService liveLogcatService;
    private volatile File liveLogcatFile;
    private volatile boolean liveCaptureActive;
    private volatile boolean liveStopRequested;
    private final Handler mainHandler = new Handler(Looper.getMainLooper());
    private final Shizuku.OnRequestPermissionResultListener shizukuPermissionListener =
            (requestCode, grantResult) -> {
                if (requestCode != SHIZUKU_LOGCAT_PERMISSION) {
                    return;
                }
                if (grantResult == PackageManager.PERMISSION_GRANTED) {
                    bindLogcatService();
                } else {
                    LOGCAT_CAPTURE.set(false);
                    showLogcatMessage("Shizuku não autorizou o GLUE 5 a ler os logs do próprio processo.");
                }
            };
    private final ServiceConnection logcatServiceConnection = new ServiceConnection() {
        @Override
        public void onServiceConnected(ComponentName name, IBinder binder) {
            if (binder == null || !binder.pingBinder()) {
                LOGCAT_CAPTURE.set(false);
                showLogcatMessage("O serviço Shizuku não entregou uma conexão válida.");
                return;
            }
            if (requestedLogcatMode == LOGCAT_LIVE) {
                LOGCAT_WORKER.execute(() -> startLiveLogcatFromService(binder));
            } else {
                LOGCAT_WORKER.execute(() -> captureLogcatFromService(binder));
            }
        }

        @Override
        public void onServiceDisconnected(ComponentName name) {
            liveLogcatService = null;
            if (liveCaptureActive) {
                liveStopRequested = true;
                File file = liveLogcatFile;
                if (file != null && file.isFile()) {
                    runOnUiThread(() -> showLogcatMessage("Shizuku disconnected; partial logcat is saved at " + file.getAbsolutePath()));
                } else {
                    showLogcatMessage("Shizuku disconnected before the live logcat could be saved.");
                }
            } else {
                LOGCAT_CAPTURE.set(false);
            }
        }
    };

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        // The same switches as a desktop run, RUST_LOG and GLUE5_*: string
        // extras of the launch intent (`mise run android:run` passes them),
        // set in the environment before SDL starts the library's main.
        Bundle extras = getIntent().getExtras();
        if (extras != null) {
            for (String key : extras.keySet()) {
                String value = extras.getString(key);
                if (value != null && (key.equals("RUST_LOG") || key.startsWith("GLUE5_"))) {
                    try {
                        Os.setenv(key, value, true);
                    } catch (ErrnoException e) {
                        // Left unset.
                    }
                }
            }
        }
        // The hardware model is distinct from the user-editable device name.
        // Glue5 uses this exact model family to gate the S21 Plus-only touch
        // profile and its motion sensors.
        try {
            Os.setenv("GLUE5_DEVICE_MODEL", Build.MODEL, true);
        } catch (ErrnoException e) {
            // Five Star controls remain unavailable when the model is unknown.
        }
        // A profile of the game (a build instrumented for PGO, said by
        // GLUE5_PROFILE): the compiler's profile runtime reads the variable
        // when the library is loaded, so it is set here, before SDL starts
        // it. The file goes to the app's own folder in the shared storage,
        // where the computer can pull it from (`mise run pgo:android`).
        if (Os.getenv("GLUE5_PROFILE") != null) {
            File external = getExternalFilesDir(null);
            if (external != null) {
                File pgo = new File(external, "pgo");
                if (pgo.isDirectory() || pgo.mkdirs()) {
                    try {
                        Os.setenv("LLVM_PROFILE_FILE", new File(pgo, "game-%m-%p.profraw").getAbsolutePath(), true);
                    } catch (ErrnoException e) {
                        // The profile goes to the default place.
                    }
                }
            }
        }
        // The device's name as its owner set it (Settings › About phone),
        // for the Studio's pages (device.rs), unless the intent named one.
        if (Os.getenv("GLUE5_DEVICE_NAME") == null) {
            String name = Settings.Global.getString(getContentResolver(), Settings.Global.DEVICE_NAME);
            if (name == null || name.isEmpty()) {
                name = Build.MODEL;
            }
            try {
                Os.setenv("GLUE5_DEVICE_NAME", name, true);
            } catch (ErrnoException e) {
                // The kind of device, then.
            }
        }
        SharedPreferences logcatPrefs = getSharedPreferences(LOGCAT_PREFS, MODE_PRIVATE);
        boolean previousSessionWasActive = logcatPrefs.getBoolean(SESSION_ACTIVE, false);
        int previousSessionPid = logcatPrefs.getInt(SESSION_PID, 0);
        SharedPreferences.Editor session = logcatPrefs.edit();
        if (previousSessionWasActive) {
            session.putBoolean(PENDING_CRASH_LOGCAT, true).putInt(PENDING_CRASH_PID, previousSessionPid);
        }
        session.putBoolean(SESSION_ACTIVE, true).putInt(SESSION_PID, android.os.Process.myPid()).commit();
        super.onCreate(savedInstanceState);
        active = this;
        Shizuku.addRequestPermissionResultListener(shizukuPermissionListener);
        resumePendingImport();
        mainHandler.postDelayed(this::promptPendingCrashLogcat, 900);
        // Android 11 and later tell where the on-screen keyboard is
        // (Glue5Surface): the app's pages end above it, so the window stays
        // put. Android 10 pans the window for the text being typed.
        if (Build.VERSION.SDK_INT >= 30 /* Android 11 (R) */) {
            getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING);
        }
    }

    @Override
    protected void onPause() {
        releaseLanLockForPause();
        super.onPause();
    }

    @Override
    protected void onDestroy() {
        releaseLanLockForDestroy();
        getSharedPreferences(LOGCAT_PREFS, MODE_PRIVATE)
                .edit()
                .putBoolean(SESSION_ACTIVE, false)
                .remove(SESSION_PID)
                .commit();
        if (LOGCAT_CAPTURE.get() && requestedLogcatMode == LOGCAT_LIVE) {
            liveStopRequested = true;
            ILogcatCaptureService service = liveLogcatService;
            if (service != null) {
                try {
                    service.stopLogcat();
                } catch (Exception error) {
                    Log.w("glue5", "could not stop live logcat while closing activity", error);
                }
            }
        }
        Shizuku.removeRequestPermissionResultListener(shizukuPermissionListener);
        if (active == this) {
            active = null;
        }
        super.onDestroy();
    }

    @Override
    protected SDLSurface createSDLSurface(Context context) {
        return new Glue5Surface(context);
    }

    /**
     * From keyboard.rs: 1, the window pans for the text being typed (the
     * game's text box, which the game draws where it does); 0, it stays put
     * (the app's pages end above the keyboard). Sent before the keyboard
     * shows, through the queue SDL shows it with.
     */
    private static final int COMMAND_KEYBOARD_PANS = COMMAND_USER;

    @Override
    protected boolean onUnhandledMessage(int command, Object param) {
        if (command == COMMAND_KEYBOARD_PANS) {
            if (Build.VERSION.SDK_INT >= 30 /* Android 11 (R) */) {
                boolean pans = param instanceof Integer && (Integer) param != 0;
                getWindow().setSoftInputMode(pans
                        ? WindowManager.LayoutParams.SOFT_INPUT_ADJUST_PAN
                        : WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING);
            }
            return true;
        }
        return super.onUnhandledMessage(command, param);
    }

    @Override
    protected String[] getLibraries() {
        return new String[] { "SDL3", "glue5" };
    }

    /** One system-tuned tick for a pressed on-screen control. */
    /** Acquire the Wi-Fi multicast lock while LAN discovery or an online session needs it. */
    public static boolean lanDiscovery(boolean on) {
        synchronized (LAN_LOCK_MONITOR) {
            lanDiscoveryRequested = on;
            return setLanMulticastLock(on);
        }
    }

    private static boolean setLanMulticastLock(boolean on) {
        if (!on) {
            try {
                if (lanMulticastLock != null && lanMulticastLock.isHeld()) {
                    lanMulticastLock.release();
                }
                return true;
            } catch (RuntimeException error) {
                Log.w("glue5", "could not release LAN multicast lock", error);
                return false;
            }
        }
        if (lanMulticastLock == null) {
            Glue5Activity activity = active;
            if (activity == null) {
                return false;
            }
            WifiManager wifi = (WifiManager) activity.getApplicationContext()
                    .getSystemService(Context.WIFI_SERVICE);
            if (wifi == null) {
                Log.w("glue5", "Wi-Fi service unavailable for LAN discovery");
                return false;
            }
            lanMulticastLock = wifi.createMulticastLock("glue5");
            lanMulticastLock.setReferenceCounted(false);
        }
        try {
            if (!lanMulticastLock.isHeld()) {
                lanMulticastLock.acquire();
            }
            return lanMulticastLock.isHeld();
        } catch (RuntimeException error) {
            Log.w("glue5", "could not acquire LAN multicast lock", error);
            return false;
        }
    }

    private static void releaseLanLockForPause() {
        synchronized (LAN_LOCK_MONITOR) {
            setLanMulticastLock(false);
        }
    }

    private static void restoreLanLockAfterResume() {
        synchronized (LAN_LOCK_MONITOR) {
            if (lanDiscoveryRequested) {
                setLanMulticastLock(true);
            }
        }
    }

    private static void releaseLanLockForDestroy() {
        synchronized (LAN_LOCK_MONITOR) {
            lanDiscoveryRequested = false;
            setLanMulticastLock(false);
        }
    }

    public static void hapticTap() {
        Glue5Activity activity = active;
        if (activity == null) {
            return;
        }
        Vibrator vibrator = activity.phoneVibrator();
        if (vibrator != null && vibrator.hasVibrator()) {
            activity.vibrateAsTouch(vibrator, VibrationEffect.createPredefined(VibrationEffect.EFFECT_TICK));
        }
    }

    /**
     * Maps the game's two controller motors onto this phone's single actuator.
     * Rust rate-limits these frame updates to 20 per second.  The stronger
     * motor sets amplitude; the high motor's share makes the pulse shorter.
     */
    public static void hapticRumble(int low, int high) {
        Glue5Activity activity = active;
        if (activity == null) {
            return;
        }
        Vibrator vibrator = activity.phoneVibrator();
        if (low <= 0 && high <= 0) {
            if (phoneRumbleActive && vibrator != null) {
                vibrator.cancel();
            }
            phoneRumbleActive = false;
            return;
        }
        if (vibrator == null || !vibrator.hasVibrator()) {
            phoneRumbleActive = false;
            return;
        }

        int strongest = Math.max(low, high);
        int amplitude = 1 + strongest * 254 / 65535;
        int sum = low + high;
        float highShare = sum == 0 ? 0f : high / (float) sum;
        long durationMs = 35L + Math.round((1f - highShare) * 45f);
        activity.vibrateAsTouch(vibrator, VibrationEffect.createOneShot(durationMs, amplitude));
        phoneRumbleActive = true;
    }

    private Vibrator phoneVibrator() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            VibratorManager manager = (VibratorManager) getSystemService(Context.VIBRATOR_MANAGER_SERVICE);
            return manager == null ? null : manager.getDefaultVibrator();
        }
        return (Vibrator) getSystemService(Context.VIBRATOR_SERVICE);
    }

    private void vibrateAsTouch(Vibrator vibrator, VibrationEffect effect) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            VibrationAttributes attributes = new VibrationAttributes.Builder()
                    .setUsage(VibrationAttributes.USAGE_TOUCH)
                    .build();
            vibrator.vibrate(effect, attributes); // Android applies the user's touch-haptics setting.
        } else if (touchHapticsEnabled()) {
            // Before API 33, VibrationAttributes.USAGE_TOUCH is unavailable;
            // explicitly honor the system's Touch feedback switch.
            vibrator.vibrate(effect);
        }
    }

    private boolean touchHapticsEnabled() {
        try {
            return Settings.System.getInt(
                    getContentResolver(), Settings.System.HAPTIC_FEEDBACK_ENABLED, 1) != 0;
        } catch (RuntimeException error) {
            if (HAPTIC_SETTING_WARNING.compareAndSet(false, true)) {
                Log.w("glue5", "could not read Android's touch-haptics setting", error);
            }
            return true;
        }
    }


    /** Called from Rust.  A tree, rather than arbitrary storage permission,
     * gives the app access only to the folder the player selects. */
    public static boolean startImport(String kind) {
        Glue5Activity activity = active;
        if (activity == null
                || (!kind.equals("save") && !kind.equals("resources") && !kind.equals(GAME_KIND))) {
            return false;
        }
        activity.runOnUiThread(() -> {
            try {
                activity.openImportTree(kind);
            } catch (ActivityNotFoundException error) {
                Log.e("glue5", "no Android folder picker available for " + kind, error);
                if (GAME_KIND.equals(kind)) {
                    activity.writeGameText(GAME_ERROR, "Android could not open a folder picker. Install or enable a Files app, then try Find files again.");
                } else {
                    Toast.makeText(activity, "Could not open a folder picker", Toast.LENGTH_LONG).show();
                }
            } catch (RuntimeException error) {
                Log.e("glue5", "could not start folder picker for " + kind, error);
                if (GAME_KIND.equals(kind)) {
                    activity.writeGameText(GAME_ERROR, "Could not start the folder picker: " + error);
                } else {
                    Toast.makeText(activity, "Could not start the folder picker", Toast.LENGTH_LONG).show();
                }
            }
        });
        return true;
    }

    /** Saves and copies a bounded support report. Fatal game failures call this before the activity exits. */
    public static void copyImportDiagnostics(String report) {
        Glue5Activity activity = active;
        if (activity == null) {
            return;
        }
        String abi = Build.SUPPORTED_ABIS.length == 0 ? "unknown" : Build.SUPPORTED_ABIS[0];
        String device = "Android: " + Build.MANUFACTURER + " " + Build.MODEL
                + " (" + Build.VERSION.RELEASE + ", API " + Build.VERSION.SDK_INT + ", " + abi + ")\n";
        String complete = device + report;
        Runnable publish = () -> {
            if (report.contains("Runtime failure: ")
                    && !report.contains("Runtime failure: none recorded")) {
                activity.getSharedPreferences(LOGCAT_PREFS, MODE_PRIVATE)
                        .edit()
                        .putBoolean(PENDING_CRASH_LOGCAT, true)
                        .putInt(PENDING_CRASH_PID, android.os.Process.myPid())
                        .commit();
            }
            boolean saved = false;
            boolean copied = false;
            try {
                File external = activity.getExternalFilesDir(null);
                if (external != null) {
                    writeText(new File(external, "glue5-support-report.txt"), complete);
                    saved = true;
                }
            } catch (IOException | RuntimeException error) {
                Log.e("glue5", "cannot save support report", error);
            }
            try {
                ClipboardManager clipboard = (ClipboardManager) activity.getSystemService(Context.CLIPBOARD_SERVICE);
                if (clipboard != null) {
                    clipboard.setPrimaryClip(ClipData.newPlainText("GLUE 5 support diagnostics", complete));
                    copied = true;
                }
            } catch (RuntimeException error) {
                Log.e("glue5", "cannot copy support report", error);
            }
            String message = copied
                    ? (saved ? "Support report saved and copied" : "Support report copied")
                    : (saved ? "Support report saved; clipboard unavailable" : "Could not save support report");
            try {
                Toast.makeText(activity, message, Toast.LENGTH_LONG).show();
            } catch (RuntimeException error) {
                Log.e("glue5", "cannot show support report result", error);
            }
        };
        if (Looper.myLooper() == Looper.getMainLooper()) {
            publish.run();
            return;
        }
        CountDownLatch finished = new CountDownLatch(1);
        activity.runOnUiThread(() -> {
            try {
                publish.run();
            } finally {
                finished.countDown();
            }
        });
        try {
            if (!finished.await(3, TimeUnit.SECONDS)) {
                Log.e("glue5", "timed out saving/copying support report before activity exit");
            }
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
        }
    }

    /** Called from Rust when LAST SESSION requests logs from the previous GLUE 5 process. */
    public static void requestLogcatCapture() {
        Glue5Activity activity = active;
        if (activity != null) {
            activity.snapshotLogcatPid = activity.getSharedPreferences(LOGCAT_PREFS, MODE_PRIVATE)
                    .getInt(PENDING_CRASH_PID, 0);
            activity.runOnUiThread(activity::showLogcatConsent);
        }
    }

    /** Called from Rust when the Settings diagnostics tab starts a live capture. */
    public static void startLogcatCapture() {
        Glue5Activity activity = active;
        if (activity != null) {
            activity.runOnUiThread(activity::showLiveLogcatConsent);
        }
    }

    /** Called from Rust when the Settings diagnostics tab finishes a live capture. */
    public static void stopLogcatCapture() {
        Glue5Activity activity = active;
        if (activity != null) {
            activity.runOnUiThread(activity::finishLiveLogcatCapture);
        }
    }

    /** Writes a fixed, timestamped observation into this process's filtered logcat stream. */
    public static void markDebugEvent(String tag) {
        if (!"SCREEN_FROZEN".equals(tag)
                && !"AUDIO_CONTINUES".equals(tag)
                && !"CONTROLS_RESPOND".equals(tag)
                && !"CONTROLS_UNRESPONSIVE".equals(tag)) {
            Log.w("glue5-debug", "Rejected unknown debug marker");
            return;
        }
        Log.w("glue5-debug", "USER_MARKER tag=" + tag
                + " elapsedRealtimeMs=" + android.os.SystemClock.elapsedRealtime());
    }

    private void promptPendingCrashLogcat() {
        SharedPreferences prefs = getSharedPreferences(LOGCAT_PREFS, MODE_PRIVATE);
        if (!prefs.getBoolean(PENDING_CRASH_LOGCAT, false) || isFinishing()) {
            return;
        }
        snapshotLogcatPid = prefs.getInt(PENDING_CRASH_PID, 0);
        showLogcatConsent();
    }

    private void showLogcatConsent() {
        if (snapshotLogcatPid <= 0) {
            clearPendingCrashLogcat();
            showLogcatMessage("Não há PID salvo da sessão anterior; não vou coletar logs de outros apps. Use a captura manual numa próxima reprodução do GLUE 5.");
            return;
        }
        new AlertDialog.Builder(this)
                .setTitle("A sessão anterior parou")
                .setMessage("Quer capturar os logs do processo GLUE 5 da sessão anterior (PID " + snapshotLogcatPid + ") usando Shizuku? A captura é filtrada para esse processo e não inclui logs de outros apps; ainda pode conter caminhos e dados de diagnóstico emitidos pelo GLUE 5. O arquivo só será salvo/compartilhado após sua confirmação e autorização do Shizuku."
                        + "\n\nSe o Shizuku não estiver ativo, inicie-o no próprio app por Depuração sem fio. O GLUE 5 não gera nem lê códigos de pareamento.")
                .setPositiveButton("Capturar logs do GLUE 5", (dialog, which) -> requestShizukuPermission(LOGCAT_SNAPSHOT))
                .setNegativeButton("Agora não", null)
                .show();
    }

    private void showLiveLogcatConsent() {
        if (LOGCAT_CAPTURE.get()) {
            showLogcatMessage("Já existe uma captura de logcat em andamento.");
            return;
        }
        new AlertDialog.Builder(this)
                .setTitle("Capturar logs do GLUE 5?")
                .setMessage("A captura contínua é filtrada para o processo do GLUE 5; não lê logs de outros apps, mas pode incluir caminhos e dados de diagnóstico emitidos pelo app. O arquivo cresce até você tocar em Finalizar e salvar e não será compartilhado automaticamente. Confirme apenas se aceita coletar esses logs."
                        + "\n\nSe o Shizuku não estiver ativo, inicie-o no próprio app por Depuração sem fio. O GLUE 5 não cria nem lê códigos de pareamento.")
                .setPositiveButton("Concordo, iniciar", (dialog, which) -> requestShizukuPermission(LOGCAT_LIVE))
                .setNegativeButton("Cancelar", null)
                .show();
    }

    private void requestShizukuPermission(int mode) {
        if (!LOGCAT_CAPTURE.compareAndSet(false, true)) {
            showLogcatMessage("Já existe uma captura de logcat em andamento.");
            return;
        }
        requestedLogcatMode = mode;
        requestedLogcatPid = mode == LOGCAT_LIVE ? android.os.Process.myPid() : snapshotLogcatPid;
        liveStopRequested = false;
        liveCaptureActive = false;
        if (requestedLogcatPid <= 0) {
            LOGCAT_CAPTURE.set(false);
            showLogcatMessage("Não há um PID válido do processo GLUE 5; nenhum log de outros apps será coletado.");
            return;
        }
        try {
            if (Shizuku.isPreV11()) {
                LOGCAT_CAPTURE.set(false);
                showLogcatMessage("Atualize o Shizuku: esta integração requer Shizuku API v11 ou mais recente.");
                return;
            }
            if (!Shizuku.pingBinder()) {
                LOGCAT_CAPTURE.set(false);
                showStartShizukuDialog();
                return;
            }
            if (Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED) {
                bindLogcatService();
            } else {
                Shizuku.requestPermission(SHIZUKU_LOGCAT_PERMISSION);
            }
        } catch (RuntimeException error) {
            LOGCAT_CAPTURE.set(false);
            showLogcatMessage("Não foi possível solicitar acesso ao Shizuku: " + error);
        }
    }

    private void showStartShizukuDialog() {
        new AlertDialog.Builder(this)
                .setTitle("Inicie o Shizuku")
                .setMessage("O serviço do Shizuku não está ativo. Abra o Shizuku, inicie-o por Depuração sem fio e volte ao GLUE 5. Depois, autorize o acesso quando o Shizuku perguntar.")
                .setPositiveButton("Abrir Shizuku", (dialog, which) -> {
                    Intent launch = getPackageManager().getLaunchIntentForPackage("moe.shizuku.privileged.api");
                    if (launch != null) {
                        try {
                            startActivity(launch);
                        } catch (RuntimeException error) {
                            showLogcatMessage("Não foi possível abrir o Shizuku: " + error);
                        }
                    } else {
                        showLogcatMessage("Instale o Shizuku e inicie o serviço por Depuração sem fio; depois volte e tente capturar novamente.");
                    }
                })
                .setNegativeButton("Cancelar", null)
                .show();
    }

    private void bindLogcatService() {
        try {
            if (Shizuku.getVersion() < 11) {
                throw new IllegalStateException("Shizuku API v11 or newer is required");
            }
            Shizuku.bindUserService(LOGCAT_SERVICE_ARGS, logcatServiceConnection);
        } catch (RuntimeException error) {
            LOGCAT_CAPTURE.set(false);
            showLogcatMessage("Não foi possível iniciar o serviço de captura do Shizuku: " + error);
        }
    }

    private void startLiveLogcatFromService(IBinder binder) {
        File file = null;
        String failure = null;
        boolean cancelled = false;
        ILogcatCaptureService service = ILogcatCaptureService.Stub.asInterface(binder);
        ParcelFileDescriptor pipe = null;
        try {
            if (liveStopRequested) {
                throw new IOException("capture cancelled before start");
            }
            pipe = service.startLogcat(requestedLogcatPid);
            if (pipe == null) {
                throw new IOException("Shizuku returned no live logcat stream");
            }
            File directory = getExternalFilesDir(null);
            if (directory == null) {
                throw new IOException("app external files directory is unavailable");
            }
            file = new File(directory, "glue5-logcat-live-" + System.currentTimeMillis() + ".txt");
            liveLogcatFile = file;
            liveLogcatService = service;
            if (liveStopRequested) {
                service.stopLogcat();
            }
            liveCaptureActive = true;
            showLogcatMessage("Capturing GLUE 5 process logs only. Return here and choose Finish and save when done.");
            String header = "GLUE 5 process logcat capture\n"
                    + "Started: " + new java.util.Date() + "\n"
                    + "Process: local.glue5 (PID " + requestedLogcatPid + ")\n"
                    + "Buffers: all; format: threadtime; filtered to this process\n\n";
            try (InputStream input = new ParcelFileDescriptor.AutoCloseInputStream(pipe);
                    OutputStream output = new BufferedOutputStream(new FileOutputStream(file))) {
                output.write(header.getBytes(java.nio.charset.StandardCharsets.UTF_8));
                output.flush();
                byte[] buffer = new byte[32 * 1024];
                int read;
                while ((read = input.read(buffer)) != -1) {
                    output.write(buffer, 0, read);
                    output.flush();
                }
            }
            if (!liveStopRequested) {
                failure = "The Shizuku logcat stream ended before Finish and save was pressed.";
            }
        } catch (Exception error) {
            if (liveStopRequested && file == null) {
                cancelled = true;
            } else {
                failure = error.toString();
            }
        } finally {
            if (pipe != null) {
                try {
                    pipe.close();
                } catch (IOException ignored) {
                    // The input stream usually closed it already.
                }
            }
            if (service != null && !liveStopRequested && file != null) {
                try {
                    service.stopLogcat();
                } catch (Exception ignored) {
                    // The Shizuku service may already have stopped.
                }
            }
            try {
                Shizuku.unbindUserService(LOGCAT_SERVICE_ARGS, logcatServiceConnection, true);
            } catch (RuntimeException ignored) {
                // Shizuku may already have stopped the user service.
            }
            liveCaptureActive = false;
            liveLogcatService = null;
            LOGCAT_CAPTURE.set(false);
        }

        File captured = file;
        String captureFailure = failure;
        boolean wasCancelled = cancelled;
        runOnUiThread(() -> {
            if (isFinishing()) {
                if (captured != null && captured.isFile()) {
                    showLogcatMessage("Live logcat stopped as the app closed; partial file saved at " + captured.getAbsolutePath());
                }
                return;
            }
            if (wasCancelled) {
                showLogcatMessage("Logcat capture cancelled before it started.");
            } else if (captured == null || !captured.isFile()) {
                showLogcatMessage("Could not save live logcat: " + captureFailure);
            } else if (hasLogcatCaptureError(captured)) {
                showLogcatMessage("Live logcat ended with an error. Partial file saved at " + captured.getAbsolutePath());
                showCapturedLogcat(captured, false);
            } else if (captureFailure != null) {
                showLogcatMessage("Live logcat ended unexpectedly; the partial file was saved.");
                showCapturedLogcat(captured, false);
            } else {
                showCapturedLogcat(captured, false);
            }
        });
    }

    private void finishLiveLogcatCapture() {
        if (!LOGCAT_CAPTURE.get() || requestedLogcatMode != LOGCAT_LIVE) {
            showLogcatMessage("No live logcat capture is active.");
            return;
        }
        liveStopRequested = true;
        ILogcatCaptureService service = liveLogcatService;
        if (service != null) {
            try {
                service.stopLogcat();
            } catch (Exception error) {
                showLogcatMessage("Could not stop the Shizuku logcat process cleanly: " + error);
            }
        }
        if (liveCaptureActive) {
            showLogcatMessage("Finishing logcat and saving the file…");
        } else {
            showLogcatMessage("Stopping the logcat capture request…");
        }
    }

    private void captureLogcatFromService(IBinder binder) {
        File file = null;
        String failure = null;
        try {
            ILogcatCaptureService service = ILogcatCaptureService.Stub.asInterface(binder);
            ParcelFileDescriptor pipe = service.captureLogcat(requestedLogcatPid);
            if (pipe == null) {
                throw new IOException("Shizuku returned no logcat stream");
            }
            File directory = getExternalFilesDir(null);
            if (directory == null) {
                throw new IOException("app external files directory is unavailable");
            }
            file = new File(directory, "glue5-logcat-" + System.currentTimeMillis() + ".txt");
            try (InputStream input = new ParcelFileDescriptor.AutoCloseInputStream(pipe);
                    OutputStream output = new BufferedOutputStream(new FileOutputStream(file))) {
                String header = "GLUE 5 process logcat snapshot\n"
                        + "Captured: " + new java.util.Date() + "\n"
                        + "Process: local.glue5 (PID " + requestedLogcatPid + ")\n"
                        + "Buffers: all; format: threadtime; filtered to this process\n\n";
                output.write(header.getBytes(java.nio.charset.StandardCharsets.UTF_8));
                byte[] buffer = new byte[64 * 1024];
                int read;
                while ((read = input.read(buffer)) != -1) {
                    output.write(buffer, 0, read);
                }
            }
        } catch (Exception error) {
            failure = error.toString();
        } finally {
            try {
                Shizuku.unbindUserService(LOGCAT_SERVICE_ARGS, logcatServiceConnection, true);
            } catch (RuntimeException ignored) {
                // Shizuku may already have stopped the one-shot service.
            }
            LOGCAT_CAPTURE.set(false);
        }

        File captured = file;
        String captureFailure = failure;
        runOnUiThread(() -> {
            if (captureFailure != null || captured == null || !captured.isFile()) {
                showLogcatMessage("Falha ao capturar o logcat via Shizuku: " + captureFailure);
            } else if (hasLogcatCaptureError(captured)) {
                showLogcatMessage("O filtro de logs retornou um erro. O arquivo parcial foi salvo em " + captured.getAbsolutePath());
            } else {
                clearPendingCrashLogcat();
                showCapturedLogcat(captured);
            }
        });
    }

    private void clearPendingCrashLogcat() {
        getSharedPreferences(LOGCAT_PREFS, MODE_PRIVATE)
                .edit()
                .remove(PENDING_CRASH_LOGCAT)
                .remove(PENDING_CRASH_PID)
                .apply();
        snapshotLogcatPid = 0;
    }

    private static boolean hasLogcatCaptureError(File file) {
        try (java.io.RandomAccessFile input = new java.io.RandomAccessFile(file, "r")) {
            long start = Math.max(0, input.length() - 4096);
            input.seek(start);
            byte[] bytes = new byte[(int) (input.length() - start)];
            input.readFully(bytes);
            return new String(bytes, java.nio.charset.StandardCharsets.UTF_8)
                    .contains("GLUE5_LOGCAT_ERROR:");
        } catch (IOException ignored) {
            return false;
        }
    }

    private void showCapturedLogcat(File file) {
        showCapturedLogcat(file, true);
    }

    private void showCapturedLogcat(File file, boolean copySmallCaptureAutomatically) {
        long size = file.length();
        String sizeText = String.format(java.util.Locale.ROOT, "%.2f MiB", size / (1024.0 * 1024.0));
        String message = "Os logs filtrados do processo GLUE 5 foram salvos em:\n" + file.getAbsolutePath()
                + "\nTamanho: " + sizeText;
        if (size <= MAX_CLIPBOARD_BYTES && copySmallCaptureAutomatically) {
            try {
                copyTextToClipboard(readText(file), "GLUE 5 process log");
                message += "\n\nOs logs do processo GLUE 5 também foram copiados para a área de transferência.";
            } catch (IOException | RuntimeException error) {
                message += "\n\nNão foi possível copiar para a área de transferência: " + error;
            }
            new AlertDialog.Builder(this)
                    .setTitle("Logs do GLUE 5 capturados")
                    .setMessage(message)
                    .setPositiveButton("Compartilhar arquivo de logs", (dialog, which) -> shareLogcat(file))
                    .setNegativeButton("Fechar", null)
                    .show();
        } else if (size <= MAX_CLIPBOARD_BYTES) {
            new AlertDialog.Builder(this)
                    .setTitle("Logcat salvo")
                    .setMessage(message + "\n\nOs logs não foram copiados nem compartilhados automaticamente.")
                    .setPositiveButton("Compartilhar arquivo de logs", (dialog, which) -> shareLogcat(file))
                    .setNeutralButton("Copiar para a área de transferência", (dialog, which) -> copyLogcatFile(file))
                    .setNegativeButton("Fechar", null)
                    .show();
        } else {
            new AlertDialog.Builder(this)
                    .setTitle("Logs do GLUE 5 capturados")
                    .setMessage(message + "\n\nO arquivo filtrado é grande demais para copiar de uma vez. Você pode compartilhar os logs do GLUE 5 ou copiar os últimos 256 KiB.")
                    .setPositiveButton("Compartilhar arquivo de logs", (dialog, which) -> shareLogcat(file))
                    .setNeutralButton("Copiar trecho final", (dialog, which) -> copyLogcatTail(file))
                    .setNegativeButton("Fechar", null)
                    .show();
        }
    }

    private void copyLogcatFile(File file) {
        try {
            copyTextToClipboard(readText(file), "GLUE 5 process log");
            Toast.makeText(this, "Logs do GLUE 5 copiados para a área de transferência.", Toast.LENGTH_LONG).show();
        } catch (IOException | RuntimeException error) {
            showLogcatMessage("Não foi possível copiar o logcat: " + error);
        }
    }

    private static String readText(File file) throws IOException {
        try (InputStream input = new java.io.FileInputStream(file);
                java.io.ByteArrayOutputStream output = new java.io.ByteArrayOutputStream()) {
            byte[] buffer = new byte[32 * 1024];
            int read;
            while ((read = input.read(buffer)) != -1) {
                output.write(buffer, 0, read);
            }
            return output.toString("UTF-8");
        }
    }

    private void copyLogcatTail(File file) {
        try (java.io.RandomAccessFile input = new java.io.RandomAccessFile(file, "r")) {
            long start = Math.max(0, input.length() - MAX_CLIPBOARD_BYTES);
            input.seek(start);
            byte[] bytes = new byte[(int) (input.length() - start)];
            input.readFully(bytes);
            copyTextToClipboard("[Final dos logs do processo GLUE 5; arquivo salvo em " + file.getAbsolutePath() + "]\n"
                    + new String(bytes, java.nio.charset.StandardCharsets.UTF_8), "GLUE 5 process log tail");
            Toast.makeText(this, "Trecho final copiado; o arquivo filtrado continua salvo.", Toast.LENGTH_LONG).show();
        } catch (IOException | RuntimeException error) {
            showLogcatMessage("Não foi possível copiar o trecho final: " + error);
        }
    }

    private void copyTextToClipboard(String text, String label) {
        ClipboardManager clipboard = (ClipboardManager) getSystemService(Context.CLIPBOARD_SERVICE);
        if (clipboard == null) {
            throw new IllegalStateException("clipboard service is unavailable");
        }
        clipboard.setPrimaryClip(ClipData.newPlainText(label, text));
    }

    private void shareLogcat(File file) {
        try {
            Uri uri = FileProvider.getUriForFile(this, getPackageName() + ".files", file);
            Intent share = new Intent(Intent.ACTION_SEND);
            share.setType("text/plain");
            share.putExtra(Intent.EXTRA_STREAM, uri);
            share.setClipData(ClipData.newUri(getContentResolver(), file.getName(), uri));
            share.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
            startActivity(Intent.createChooser(share, "Compartilhar logs do GLUE 5"));
        } catch (RuntimeException error) {
            showLogcatMessage("Não foi possível compartilhar o arquivo de logcat: " + error);
        }
    }

    private void showLogcatMessage(String message) {
        runOnUiThread(() -> Toast.makeText(this, message, Toast.LENGTH_LONG).show());
    }

    private void openImportTree(String kind) {
        SharedPreferences prefs = getSharedPreferences(IMPORT_PREFS, MODE_PRIVATE);
        prefs.edit().putString(PENDING_KIND, kind).remove(PENDING_URI).apply();
        if (kind.equals(GAME_KIND)) {
            clearGamePick();
            openGameTree();
            return;
        }
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
        intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
        startActivityForResult(intent, IMPORT_TREE);
    }

    /**
     * The folder the game is played from, chosen rather than copied: its
     * files are the engine's 20 GB, read where they are.
     *
     * A document tree's grant reaches files through the provider, one binder
     * call an open, which the engine's reads (and the paths its VFS takes)
     * are not written for; and copying that much into the app's own folder
     * needs the room twice over.  So the folder is read by its path, which
     * takes all-files access: asked for first, and the picker goes on once
     * the player is back from the system's page for it.
     */
    private void openGameTree() {
        SharedPreferences prefs = getSharedPreferences(IMPORT_PREFS, MODE_PRIVATE);
        if (!Environment.isExternalStorageManager()) {
            Toast.makeText(this, "Allow Glue 5 to access all files, then choose the game folder.", Toast.LENGTH_LONG).show();
            askAllFilesAccess();
            return;
        }
        prefs.edit().remove(PENDING_KIND).apply();
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
        intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
        try {
            startActivityForResult(intent, GAME_TREE);
        } catch (ActivityNotFoundException error) {
            Log.e("glue5", "no Android folder picker available for game files", error);
            writeGameText(GAME_ERROR, "Android could not open a folder picker. " + GAME_FILES_HELP);
        } catch (RuntimeException error) {
            Log.e("glue5", "could not start Android folder picker for game files", error);
            writeGameText(GAME_ERROR, "Could not start the Android folder picker: " + error
                    + ". " + GAME_FILES_HELP);
        }
    }

    /** Android 11's system page for this app's all-files access; it answers
     * to {@link #onResume}, not with a result. */
    private void askAllFilesAccess() {
        Uri self = Uri.parse("package:" + getPackageName());
        Intent intent = new Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, self);
        try {
            startActivityForResult(intent, ALL_FILES);
        } catch (ActivityNotFoundException ignored) {
            // A system without that page: its general one, or nothing.
            try {
                startActivityForResult(new Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION), ALL_FILES);
            } catch (RuntimeException also) {
                getSharedPreferences(IMPORT_PREFS, MODE_PRIVATE).edit().clear().apply();
                writeGameText(GAME_ERROR, "Could not open Android's all-files access settings. " + GAME_FILES_HELP);
            }
        } catch (RuntimeException error) {
            getSharedPreferences(IMPORT_PREFS, MODE_PRIVATE).edit().clear().apply();
            writeGameText(GAME_ERROR, "Could not open Android's all-files access settings: " + error
                    + ". " + GAME_FILES_HELP);
        }
    }

    @Override
    protected void onResume() {
        super.onResume();
        restoreLanLockAfterResume();
        SharedPreferences prefs = getSharedPreferences(IMPORT_PREFS, MODE_PRIVATE);
        if (!GAME_KIND.equals(prefs.getString(PENDING_KIND, null))) {
            return;
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R && Environment.isExternalStorageManager()) {
            // Back from the system's page with it: on to the picker.
            openGameTree();
        } else {
            prefs.edit().clear().apply();
            writeGameText(GAME_ERROR, "Find files needs All files access. Enable it in Android settings. "
                    + GAME_FILES_HELP);
        }
    }

    /** The folder the player chose for the game, left for Rust to try. */
    private void gameTreeChosen(int resultCode, Intent data) {
        SharedPreferences prefs = getSharedPreferences(IMPORT_PREFS, MODE_PRIVATE);
        prefs.edit().clear().apply();
        if (resultCode != RESULT_OK || data == null || data.getData() == null) {
            writeGameText(GAME_ERROR, "No game folder was selected. Tap Find files and choose the folder containing the game.");
            return;
        }
        File dir;
        try {
            dir = treePath(data.getData());
        } catch (RuntimeException error) {
            writeGameText(GAME_ERROR, "Android could not read the selected folder: " + error);
            return;
        }
        if (dir == null) {
            writeGameText(GAME_ERROR, "That folder has no path of its own (a cloud one, perhaps). "
                    + "Choose a folder on this phone's storage.");
            return;
        }
        if (!dir.isDirectory() || !dir.canRead()) {
            writeGameText(GAME_ERROR, dir + " is not a readable folder. Grant all-files access and choose a folder on local device storage.");
            return;
        }
        // Whether it has the game is Rust's call, which knows this build's
        // layout; a folder it cannot read shows there too.
        writeGameText(GAME_PATH, dir.getAbsolutePath());
    }

    /**
     * Where a chosen tree is on the storage: the local provider's ids are
     * paths under a volume.  None for a provider with no path of its own
     * (a cloud one), whose files the engine could not read anyway.
     */
    private static File treePath(Uri tree) {
        if (!"com.android.externalstorage.documents".equals(tree.getAuthority())) {
            return null;
        }
        String id = DocumentsContract.getTreeDocumentId(tree);
        int colon = id.indexOf(':');
        if (colon <= 0) {
            return null;
        }
        String volume = id.substring(0, colon);
        String rest = id.substring(colon + 1);
        if ("primary".equals(volume)) {
            return new File(Environment.getExternalStorageDirectory(), rest);
        }
        // An SD card or a USB drive, by its volume's name.
        return new File("/storage/" + volume, rest);
    }

    /** Removes a stale picker result so canceling a new attempt cannot reuse an old folder. */
    private void clearGamePick() {
        File external = getExternalFilesDir(null);
        if (external == null) {
            return;
        }
        File imports = new File(external, "imports");
        new File(imports, GAME_PATH).delete();
        new File(imports, GAME_ERROR).delete();
    }

    /** Leaves an answer for Rust, which reads it while the start page is up. */
    private void writeGameText(String name, String text) {
        File external = getExternalFilesDir(null);
        if (external == null) {
            Log.e("glue5", "cannot record Find files result: external app directory is unavailable");
            return;
        }
        File imports = new File(external, "imports");
        if (!imports.isDirectory() && !imports.mkdirs() && !imports.isDirectory()) {
            Log.e("glue5", "cannot create the imports directory for Find files");
            return;
        }
        try {
            writeText(new File(imports, name), text);
            Log.i("glue5", "Find files result recorded: " + name);
        } catch (IOException error) {
            Log.e("glue5", "cannot record Find files result: " + name, error);
        }
    }

    @Override
    @SuppressWarnings("deprecation")
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode == GAME_TREE) {
            gameTreeChosen(resultCode, data);
            return;
        }
        if (requestCode == ALL_FILES) {
            // The system's page answers to onResume, not here; a result is
            // only another chance to see what the player decided.
            return;
        }
        if (requestCode != IMPORT_TREE) {
            return;
        }
        SharedPreferences prefs = getSharedPreferences(IMPORT_PREFS, MODE_PRIVATE);
        if (resultCode != RESULT_OK || data == null || data.getData() == null) {
            prefs.edit().clear().apply();
            return;
        }
        Uri tree = data.getData();
        try {
            getContentResolver().takePersistableUriPermission(tree, Intent.FLAG_GRANT_READ_URI_PERMISSION);
        } catch (SecurityException ignored) {
            // Some document providers do not offer a persistable grant. The
            // result can still be copied now while this process is alive.
        }
        prefs.edit().putString(PENDING_URI, tree.toString()).apply();
        resumePendingImport();
    }

    /** If Android killed the renderer while the picker was visible, the URI
     * and chosen kind are still here when the activity is made again. */
    private void resumePendingImport() {
        SharedPreferences prefs = getSharedPreferences(IMPORT_PREFS, MODE_PRIVATE);
        String kind = prefs.getString(PENDING_KIND, null);
        String saved = prefs.getString(PENDING_URI, null);
        if (kind == null || saved == null || !COPYING.compareAndSet(false, true)) {
            return;
        }
        IMPORTS.execute(() -> copyPendingImport(kind, Uri.parse(saved)));
    }

    private void copyPendingImport(String kind, Uri tree) {
        String token = kind + "-" + UUID.randomUUID();
        String selectedName = displayName(tree);
        String selectedId;
        try {
            selectedId = DocumentsContract.getTreeDocumentId(tree);
        } catch (Exception error) {
            selectedId = "";
        }
        Log.i("glue5", "import: copying kind=" + kind + " selected-folder=" + selectedName + " tree-id=" + selectedId);
        File external = getExternalFilesDir(null);
        if (external == null) {
            Log.e("glue5", "import: app external files directory is unavailable");
            COPYING.set(false);
            return;
        }
        File root = new File(external, "imports");
        File partial = new File(new File(root, "partial"), token);
        try {
            deleteTree(partial);
            if (!partial.mkdirs() && !partial.isDirectory()) {
                throw new IOException("cannot create import staging folder");
            }
            writeText(new File(partial, ".glue5-source-name"), selectedName);
            writeText(new File(partial, ".glue5-source-document-id"), selectedId);
            copyTree(tree, partial);
            File ready = new File(new File(root, "ready"), token);
            if (!ready.getParentFile().mkdirs() && !ready.getParentFile().isDirectory()) {
                throw new IOException("cannot create completed imports folder");
            }
            if (!partial.renameTo(ready)) {
                throw new IOException("cannot finalize imported folder");
            }
            Log.i("glue5", "import: copied selected-folder=" + selectedName + " tree-id=" + selectedId
                    + " staging=" + ready.getAbsolutePath());
            getSharedPreferences(IMPORT_PREFS, MODE_PRIVATE).edit().clear().apply();
        } catch (Exception error) {
            Log.e("glue5", "import: failed kind=" + kind + " selected-folder=" + selectedName
                    + " tree-id=" + selectedId, error);
            try {
                File errors = new File(root, "errors");
                errors.mkdirs();
                writeText(new File(errors, token + ".txt"), error + " (selected folder: " + selectedName
                        + "; document id: " + selectedId + ")");
            } catch (IOException ignored) {
            }
            deleteTree(partial);
            getSharedPreferences(IMPORT_PREFS, MODE_PRIVATE).edit().clear().apply();
        } finally {
            COPYING.set(false);
        }
    }

    /** Copies descendants only.  Document providers supply opaque IDs, so
     * this cannot follow a path outside the folder selected by the player. */
    private void copyTree(Uri tree, File destination) throws IOException {
        ContentResolver resolver = getContentResolver();
        String parentId = DocumentsContract.getTreeDocumentId(tree);
        Uri children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, parentId);
        String[] columns = {
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE
        };
        try (Cursor cursor = resolver.query(children, columns, null, null, null)) {
            if (cursor == null) {
                throw new IOException("the selected folder cannot be read");
            }
            int id = cursor.getColumnIndexOrThrow(DocumentsContract.Document.COLUMN_DOCUMENT_ID);
            int name = cursor.getColumnIndexOrThrow(DocumentsContract.Document.COLUMN_DISPLAY_NAME);
            int mime = cursor.getColumnIndexOrThrow(DocumentsContract.Document.COLUMN_MIME_TYPE);
            while (cursor.moveToNext()) {
                String childName = safeName(cursor.getString(name));
                String childId = cursor.getString(id);
                String type = cursor.getString(mime);
                Uri child = DocumentsContract.buildDocumentUriUsingTree(tree, childId);
                File out = new File(destination, childName);
                if (DocumentsContract.Document.MIME_TYPE_DIR.equals(type)) {
                    if (!out.mkdirs() && !out.isDirectory()) {
                        throw new IOException("cannot create " + childName);
                    }
                    copyTree(child, out);
                } else {
                    copyFile(resolver, child, out);
                }
            }
        }
    }

    private String displayName(Uri tree) {
        String documentId = null;
        try {
            documentId = DocumentsContract.getTreeDocumentId(tree);
            Uri document = DocumentsContract.buildDocumentUriUsingTree(tree, documentId);
            try (Cursor cursor = getContentResolver().query(document,
                    new String[] { DocumentsContract.Document.COLUMN_DISPLAY_NAME }, null, null, null)) {
                if (cursor != null && cursor.moveToFirst()) {
                    return safeName(cursor.getString(0));
                }
            }
        } catch (Exception ignored) {
            // Some providers do not return metadata for the tree root.
        }
        // The external-storage provider encodes the selected folder's path in
        // its document ID (for example primary:.../Profiles/0F74F4C4).
        if (documentId != null
                && "com.android.externalstorage.documents".equals(tree.getAuthority())) {
            String path = documentId;
            int volume = path.indexOf(':');
            if (volume >= 0) {
                path = path.substring(volume + 1);
            }
            path = path.replace('\\', '/');
            while (path.endsWith("/")) {
                path = path.substring(0, path.length() - 1);
            }
            String name = path.substring(path.lastIndexOf('/') + 1);
            try {
                return safeName(name);
            } catch (IOException ignored) {
                // Rust refuses to install if no genuine profile name is found.
            }
        }
        return "";
    }

    private static String safeName(String value) throws IOException {
        if (value == null || value.isEmpty() || value.equals(".") || value.equals("..")
                || value.indexOf('/') >= 0 || value.indexOf('\\') >= 0) {
            throw new IOException("unsafe file name from document provider");
        }
        return value;
    }

    private static void copyFile(ContentResolver resolver, Uri source, File target) throws IOException {
        InputStream raw = resolver.openInputStream(source);
        if (raw == null) {
            throw new IOException("cannot open selected file");
        }
        try (InputStream input = new BufferedInputStream(raw);
             OutputStream output = new BufferedOutputStream(new FileOutputStream(target))) {
            byte[] buffer = new byte[64 * 1024];
            for (int read; (read = input.read(buffer)) >= 0;) {
                output.write(buffer, 0, read);
            }
        }
    }

    private static void writeText(File file, String text) throws IOException {
        try (OutputStream output = new FileOutputStream(file)) {
            output.write(text.getBytes(java.nio.charset.StandardCharsets.UTF_8));
        }
    }

    private static void deleteTree(File file) {
        File[] children = file.listFiles();
        if (children != null) {
            for (File child : children) {
                deleteTree(child);
            }
        }
        if (file.exists()) {
            file.delete();
        }
    }
}
