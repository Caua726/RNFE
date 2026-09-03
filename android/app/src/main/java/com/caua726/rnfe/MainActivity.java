package com.caua726.rnfe;

import android.app.NativeActivity;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.provider.OpenableColumns;
import android.view.View;
import android.view.WindowManager;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;

/** NativeActivity + seletor de arquivos do sistema (SAF), que devolve a ROM ao Rust por JNI. */
public class MainActivity extends NativeActivity {
    private static final int PICK_ROM = 1;
    /** Maior que qualquer .nes (as maiores ROMs licenciadas têm ~1 MB; multicarts, 4 MB). */
    private static final int MAX_ROM = 8 << 20;

    static {
        System.loadLibrary("rnfe_android");
    }

    /** Implementado em Rust (crates/rnfe-android); pode ser chamado de qualquer thread. */
    public native void onRomPicked(byte[] data, String name);

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        // Jogo em andamento: a tela não pode apagar sozinha
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        hideSystemUi();
    }

    /** Vibração curta ao tocar num botão (chamada pelo Rust via JNI). */
    public void vibrate() {
        try {
            Vibrator v = (Vibrator) getSystemService(VIBRATOR_SERVICE);
            if (v != null && v.hasVibrator()) {
                v.vibrate(VibrationEffect.createOneShot(12, VibrationEffect.DEFAULT_AMPLITUDE));
            }
        } catch (Exception ignored) {
        }
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
        final Uri uri = data.getData();
        // Fora da thread principal: provedores em nuvem (Drive) baixam o arquivo aqui (ANR).
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
