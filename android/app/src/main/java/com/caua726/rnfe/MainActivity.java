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

    /** Eixos do gamepad (d-pad como hat, analógico esquerdo), -1..1. Implementado em Rust. */
    public native void onPadAxes(float x, float y);

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        // Jogo em andamento: a tela não pode apagar sozinha
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        vibrator = (Vibrator) getSystemService(VIBRATOR_SERVICE);
        hideSystemUi();
        handleViewIntent(getIntent());
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
        mainHandler.post(() -> {
            List<Rect> rects = new ArrayList<>();
            if (r1 > l1 && b1 > t1) rects.add(new Rect(l1, t1, r1, b1));
            if (r2 > l2 && b2 > t2) rects.add(new Rect(l2, t2, r2, b2));
            try {
                getWindow().getDecorView().setSystemGestureExclusionRects(rects);
            } catch (Exception ignored) {
            }
        });
    }

    @Override
    protected void onDestroy() {
        if (!isFinishing()) {
            // Recriação da Activity (mudança de configuração fora de configChanges, "Não manter
            // atividades"): o winit ignora o Destroy e nunca sai do laço; super.onDestroy()
            // esperaria por ele para sempre (ANR). Recomeçar do zero é mais barato.
            android.os.Process.killProcess(android.os.Process.myPid());
        }
        super.onDestroy();
        // Saída normal (android_main voltou → finish): o winit não recria o EventLoop no mesmo
        // processo, então encerra o processo para o próximo toque no ícone começar limpo.
        System.exit(0);
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
                onRomPicked(new byte[0], "");
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
            byte[] bytes = new byte[0];
            try (InputStream in = getContentResolver().openInputStream(uri)) {
                ByteArrayOutputStream out = new ByteArrayOutputStream();
                byte[] buf = new byte[65536];
                int n, total = 0;
                while ((n = in.read(buf)) > 0) {
                    total += n;
                    if (total > MAX_ROM) { out = null; break; } // não é uma ROM
                    out.write(buf, 0, n);
                }
                if (out != null) bytes = out.toByteArray();
            } catch (Exception ignored) {
            }
            onRomPicked(bytes, name);
        }, "rnfe-rom").start();
    }
}
