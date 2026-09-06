package com.caua726.rnfe;

import android.app.NativeActivity;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.graphics.Rect;
import android.provider.OpenableColumns;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowInsets;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityNodeProvider;
import android.widget.Button;
import java.util.Collections;
import android.widget.Toast;
import android.view.WindowManager;
import java.util.ArrayList;
import java.util.List;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;

/** NativeActivity + seletor de arquivos do sistema (SAF), que devolve a ROM ao Rust por JNI. */
public class MainActivity extends NativeActivity {
    private static final int PICK_ROM = 1;
    /** Maior que qualquer .nes (as maiores ROMs licenciadas têm ~1 MB; multicarts, 4 MB). */
    private static final int MAX_ROM = 8 << 20;

    private Vibrator vibrator;
    private final android.os.Handler mainHandler = new android.os.Handler(android.os.Looper.getMainLooper());

    static {
        System.loadLibrary("rnfe_android");
    }

    /** Implementado em Rust (crates/rnfe-android); pode ser chamado de qualquer thread. */
    public native void onRomPicked(byte[] data, String name);
    /** Motivo legível quando a ROM não pôde ser lida (o Rust mostra num aviso). */
    public native void onRomFailed(String why);

    /** Eixos do gamepad (d-pad como hat, analógico esquerdo), -1..1. Implementado em Rust. */
    public native void onPadAxes(float x, float y);

    /** O leitor de tela ativou o item de índice `i` da lista publicada. Implementado em Rust. */
    public native void onA11yClick(int index);

    /** Área segura da janela em px (recorte da câmera, barras, gestos). Implementado em Rust. */
    public native void onInsets(float left, float top, float right, float bottom);

    private A11yOverlay a11y;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        // A tela fica ligada só enquanto joga (setKeepScreenOn, chamado pelo Rust)
        vibrator = (Vibrator) getSystemService(VIBRATOR_SERVICE);
        hideSystemUi();
        // O jogo é uma superfície desenhada por nós: para o TalkBack, o app inteiro era um
        // retângulo mudo. Esta camada transparente publica os itens do menu como nós virtuais.
        a11y = new A11yOverlay(this);
        addContentView(
                a11y,
                new ViewGroup.LayoutParams(
                        ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        // Área segura real, em vez de margens chutadas em porcentagem no Rust.
        getWindow().getDecorView().setOnApplyWindowInsetsListener((v, insets) -> {
            reportInsets(insets);
            return insets;
        });
        // Reaberto pelo Recents depois de o sistema matar o processo: o VIEW original não vale mais
        if ((getIntent().getFlags() & Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY) == 0) {
            handleViewIntent(getIntent());
        }
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        handleViewIntent(intent);
    }

    /** "Abrir com": um .nes tocado em Downloads/Drive chega como ACTION_VIEW. */
    private void handleViewIntent(Intent intent) {
        if (intent == null || !Intent.ACTION_VIEW.equals(intent.getAction()) || intent.getData() == null) return;
        readRom(intent.getData());
    }

    /** Gamepads Bluetooth mandam d-pad (AXIS_HAT_X/Y) e analógico (AXIS_X/Y) como movimento. */
    @Override
    public boolean dispatchGenericMotionEvent(MotionEvent ev) {
        int src = ev.getSource();
        if ((src & InputDevice.SOURCE_JOYSTICK) == InputDevice.SOURCE_JOYSTICK
                || (src & InputDevice.SOURCE_GAMEPAD) == InputDevice.SOURCE_GAMEPAD) {
            float hx = ev.getAxisValue(MotionEvent.AXIS_HAT_X);
            float hy = ev.getAxisValue(MotionEvent.AXIS_HAT_Y);
            float x = Math.abs(hx) > 0.5f ? hx : ev.getAxisValue(MotionEvent.AXIS_X);
            float y = Math.abs(hy) > 0.5f ? hy : ev.getAxisValue(MotionEvent.AXIS_Y);
            onPadAxes(x, y);
            return true;
        }
        return super.dispatchGenericMotionEvent(ev);
    }

    /** Retângulos onde o gesto de borda do sistema não deve capturar o toque (d-pad, A/B). */
    public void setGestureExclusion(int l1, int t1, int r1, int b1, int l2, int t2, int r2, int b2) {
        if (android.os.Build.VERSION.SDK_INT < 29) return; // API 29+; antes disso é NoSuchMethodError (Error, não Exception)
        mainHandler.post(() -> {
            List<Rect> rects = new ArrayList<>();
            if (r1 > l1 && b1 > t1) rects.add(new Rect(l1, t1, r1, b1));
            if (r2 > l2 && b2 > t2) rects.add(new Rect(l2, t2, r2, b2));
            try {
                getWindow().getDecorView().setSystemGestureExclusionRects(rects);
            } catch (Throwable ignored) {
            }
        });
    }

    @Override
    protected void onDestroy() {
        // O winit não recria o laço de eventos numa Activity nova e o glue nativo bloqueia a
        // thread principal esperando a thread do jogo (ANR): o processo morre aqui, sempre.
        // save/config já foram gravados em `suspended`.
        super.onDestroy();
        android.os.Process.killProcess(android.os.Process.myPid());
    }

    /** Manda ao Rust a área segura da janela (recorte da câmera, barras e faixa de gesto). */
    private void reportInsets(WindowInsets insets) {
        if (insets == null) return;
        int l, t, r, b;
        if (android.os.Build.VERSION.SDK_INT >= 30) {
            android.graphics.Insets i = insets.getInsets(
                    WindowInsets.Type.systemBars()
                            | WindowInsets.Type.displayCutout()
                            | WindowInsets.Type.systemGestures());
            l = i.left;
            t = i.top;
            r = i.right;
            b = i.bottom;
        } else {
            l = insets.getSystemWindowInsetLeft();
            t = insets.getSystemWindowInsetTop();
            r = insets.getSystemWindowInsetRight();
            b = insets.getSystemWindowInsetBottom();
            android.view.DisplayCutout cut =
                    android.os.Build.VERSION.SDK_INT >= 28 ? insets.getDisplayCutout() : null;
            if (cut != null) {
                l = Math.max(l, cut.getSafeInsetLeft());
                t = Math.max(t, cut.getSafeInsetTop());
                r = Math.max(r, cut.getSafeInsetRight());
                b = Math.max(b, cut.getSafeInsetBottom());
            }
        }
        try {
            onInsets(l, t, r, b);
        } catch (Throwable ignored) { // o laço nativo pode ainda não existir
        }
    }

    /** Itens do menu para o leitor de tela (chamada pelo Rust, de outra thread). */
    public void setA11yNodes(String[] labels, int[] rects) {
        final A11yOverlay view = a11y;
        if (view == null) return;
        mainHandler.post(() -> view.setNodes(labels, rects));
    }

    /** Camada transparente que existe só para o leitor de tela: não desenha e não pega toque. */
    private final class A11yOverlay extends View {
        private String[] labels = new String[0];
        private int[] rects = new int[0];

        A11yOverlay(android.content.Context ctx) {
            super(ctx);
            setWillNotDraw(true);
            setClickable(false);
            setFocusable(false);
            setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_YES);
        }

        void setNodes(String[] newLabels, int[] newRects) {
            if (newLabels == null || newRects == null || newRects.length < newLabels.length * 4) {
                labels = new String[0];
                rects = new int[0];
            } else {
                labels = newLabels;
                rects = newRects;
            }
            sendAccessibilityEvent(AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED);
        }

        @Override
        public boolean onTouchEvent(MotionEvent ev) {
            return false; // o toque é do jogo, sempre
        }

        @Override
        public AccessibilityNodeProvider getAccessibilityNodeProvider() {
            return provider;
        }

        private final AccessibilityNodeProvider provider = new AccessibilityNodeProvider() {
            @Override
            public AccessibilityNodeInfo createAccessibilityNodeInfo(int id) {
                if (id == View.NO_ID) {
                    AccessibilityNodeInfo host = AccessibilityNodeInfo.obtain(A11yOverlay.this);
                    onInitializeAccessibilityNodeInfo(host);
                    for (int i = 0; i < labels.length; i++) {
                        host.addChild(A11yOverlay.this, i);
                    }
                    return host;
                }
                if (id < 0 || id >= labels.length) return null;
                AccessibilityNodeInfo info = AccessibilityNodeInfo.obtain(A11yOverlay.this, id);
                info.setPackageName(getContext().getPackageName());
                info.setClassName(Button.class.getName());
                info.setContentDescription(labels[id]);
                info.setParent(A11yOverlay.this);
                info.setSource(A11yOverlay.this, id);
                info.setVisibleToUser(true);
                info.setEnabled(true);
                info.setClickable(true);
                info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_CLICK);
                int[] loc = new int[2];
                getLocationOnScreen(loc);
                int i4 = id * 4;
                Rect screen = new Rect(rects[i4], rects[i4 + 1], rects[i4 + 2], rects[i4 + 3]);
                screen.offset(loc[0], loc[1]);
                info.setBoundsInScreen(screen);
                return info;
            }

            @Override
            public boolean performAction(int id, int action, Bundle args) {
                if (id >= 0 && id < labels.length && action == AccessibilityNodeInfo.ACTION_CLICK) {
                    onA11yClick(id);
                    return true;
                }
                return false;
            }

            @Override
            public java.util.List<AccessibilityNodeInfo> findAccessibilityNodeInfosByText(
                    String text, int id) {
                return Collections.emptyList();
            }
        };
    }

    /** Mantém a tela ligada enquanto há jogo rodando (chamada pelo Rust via JNI). */
    public void setKeepScreenOn(boolean on) {
        mainHandler.post(() -> {
            if (on) getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
            else getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        });
    }

    /** Aviso na tela quando não dá para desenhar nada (falha de GPU): senão o app fica preto
     *  e calado, e só o logcat sabe o motivo. Chamada pelo Rust via JNI. */
    public void toast(String msg) {
        if (msg == null || msg.isEmpty()) return;
        mainHandler.post(() -> {
            try {
                Toast.makeText(this, msg, Toast.LENGTH_LONG).show();
            } catch (Throwable ignored) {
            }
        });
    }

    /** Vibração curta ao tocar num botão (chamada pelo Rust via JNI, da thread de emulação). */
    public void vibrate() {
        if (vibrator == null || !vibrator.hasVibrator()) return;
        mainHandler.post(() -> {
            try {
                vibrator.vibrate(VibrationEffect.createOneShot(12, VibrationEffect.DEFAULT_AMPLITUDE));
            } catch (Exception ignored) {
            }
        });
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) hideSystemUi();
    }

    private void hideSystemUi() {
        getWindow().getDecorView().setSystemUiVisibility(
            View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
            | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
            | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_FULLSCREEN);
    }

    /** Chamado pelo Rust (JNI) para abrir o seletor. */
    public void pickRom() {
        runOnUiThread(() -> {
            Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            intent.addCategory(Intent.CATEGORY_OPENABLE);
            intent.setType("*/*");
            try {
                startActivityForResult(intent, PICK_ROM);
            } catch (Exception e) { // ActivityNotFoundException: aparelho sem seletor de arquivos
                onRomFailed("este aparelho não tem um seletor de arquivos");
            }
        });
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode != PICK_ROM) return;
        if (resultCode != RESULT_OK || data == null || data.getData() == null) {
            onRomPicked(new byte[0], "");
            return;
        }
        readRom(data.getData());
    }

    /** Lê a ROM fora da thread principal (provedores em nuvem baixam o arquivo aqui: ANR). */
    private void readRom(final Uri uri) {
        new Thread(() -> {
            String name = "rom.nes";
            try (Cursor c = getContentResolver().query(uri, null, null, null, null)) {
                if (c != null && c.moveToFirst()) {
                    int idx = c.getColumnIndex(OpenableColumns.DISPLAY_NAME);
                    if (idx >= 0) name = c.getString(idx);
                }
            } catch (Exception ignored) {
            }
            if (name == null || name.isEmpty()) {
                String seg = uri.getLastPathSegment();
                name = seg == null ? "rom.nes" : seg;
            }
            byte[] bytes;
            try (InputStream in = getContentResolver().openInputStream(uri)) {
                ByteArrayOutputStream out = new ByteArrayOutputStream();
                byte[] buf = new byte[65536];
                int n, total = 0;
                while ((n = in.read(buf)) > 0) {
                    total += n;
                    if (total > MAX_ROM) { // não é uma ROM de NES
                        onRomFailed(name + " tem mais de 8 MB: não é uma ROM de NES");
                        return;
                    }
                    out.write(buf, 0, n);
                }
                bytes = out.toByteArray();
            } catch (Exception e) {
                onRomFailed("não consegui ler " + name + " (" + e.getClass().getSimpleName() + ")");
                return;
            }
            onRomPicked(bytes, name);
        }, "rnfe-rom").start();
    }
}
