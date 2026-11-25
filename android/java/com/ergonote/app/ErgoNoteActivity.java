package com.ergonote.app;

import android.app.NativeActivity;
import android.content.ContentResolver;
import android.content.Context;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.OpenableColumns;
import android.speech.tts.TextToSpeech;
import android.speech.tts.UtteranceProgressListener;
import android.text.Editable;
import android.text.InputType;
import android.text.Selection;
import android.text.SpannableStringBuilder;
import android.util.Log;
import android.view.KeyCharacterMap;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.CompletionInfo;
import android.view.inputmethod.CorrectionInfo;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;
import android.view.inputmethod.TextAttribute;
import android.widget.FrameLayout;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.util.Locale;

public class ErgoNoteActivity extends NativeActivity {
    static {
        System.loadLibrary("ergonote_android");
    }

    private static final String TAG = "ErgoNote";
    private static final int REQ_PICK_FILE = 4201;
    private static final int REQ_TAKE_PHOTO = 4202;
    private static final int REQ_VOICE_DICTATION = 4203;
    private static final int IME_KEY_ENTER = 1;
    private static final int IME_KEY_BACKSPACE = 2;

    private static native void nativeOnFilePicked(String path);
    private static native void nativeOnFilePickCancelled();
    private static native void nativeOnVoiceDictated(String text);
    private static native void nativeOnReleaseJson(String json);
    private static native void nativeOnImeParagraph(long syncGen, String text, int selStartUtf16, int selEndUtf16);
    private static native void nativeOnImeDelta(int deleteBefore, String insertText);
    private static native void nativeOnImeKey(int keyKind);
    private static native void nativeOnTtsDone();

    private ImeProxyView imeView;
    private boolean imeVisible = false;
    private volatile long latestQueuedSyncGen = 0;
    private TextToSpeech tts;
    private boolean ttsReady = false;
    private String pendingTtsText;
    private String pendingTtsLocale;
    private float pendingTtsRate = 1.0f;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        imeView = new ImeProxyView(this);
        addContentView(imeView, new FrameLayout.LayoutParams(1, 1));
        imeView.requestFocus();
    }

    @Override
    protected void onDestroy() {
        if (tts != null) {
            tts.stop();
            tts.shutdown();
            tts = null;
        }
        super.onDestroy();
    }

    public void speakTts(final String text, final String localeTag, final float rate) {
        if (text == null || text.isEmpty()) {
            return;
        }
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                pendingTtsText = text;
                pendingTtsLocale = localeTag;
                pendingTtsRate = rate > 0.1f ? rate : 1.0f;
                if (tts == null) {
                    tts = new TextToSpeech(ErgoNoteActivity.this, new TextToSpeech.OnInitListener() {
                        @Override
                        public void onInit(int status) {
                            ttsReady = (status == TextToSpeech.SUCCESS);
                            if (ttsReady && tts != null) {
                                tts.setOnUtteranceProgressListener(new UtteranceProgressListener() {
                                    @Override
                                    public void onStart(String utteranceId) {}

                                    @Override
                                    public void onDone(String utteranceId) {
                                        nativeOnTtsDone();
                                    }

                                    @Override
                                    public void onError(String utteranceId) {
                                        nativeOnTtsDone();
                                    }
                                });
                                if (pendingTtsText != null) {
                                    speakPendingTts();
                                }
                            } else {
                                pendingTtsText = null;
                                nativeOnTtsDone();
                            }
                        }
                    });
                } else if (ttsReady) {
                    speakPendingTts();
                }
            }
        });
    }

    private void speakPendingTts() {
        if (tts == null || !ttsReady || pendingTtsText == null) {
            return;
        }
        String text = pendingTtsText;
        pendingTtsText = null;
        if (pendingTtsLocale != null && !pendingTtsLocale.isEmpty()) {
            tts.setLanguage(Locale.forLanguageTag(pendingTtsLocale));
        }
        tts.setSpeechRate(pendingTtsRate);
        tts.speak(text, TextToSpeech.QUEUE_FLUSH, null, "ergonote-tts");
    }

    public void stopTts() {
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                pendingTtsText = null;
                if (tts != null) {
                    tts.stop();
                }
            }
        });
    }

    public void openFilePicker(final String mimeType) {
        if ("camera/photo".equals(mimeType)) {
            openCamera();
            return;
        }
        if (mimeType != null && mimeType.startsWith("voice/dictation:")) {
            openVoiceDictation(mimeType.substring("voice/dictation:".length()));
            return;
        }
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                try {
                    Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                    intent.addCategory(Intent.CATEGORY_OPENABLE);
                    String type = (mimeType != null && !mimeType.isEmpty()) ? mimeType : "*/*";
                    intent.setType(type);
                    startActivityForResult(intent, REQ_PICK_FILE);
                } catch (Exception e) {
                    Log.e(TAG, "Failed to launch file picker", e);
                    nativeOnFilePickCancelled();
                }
            }
        });
    }

    public void openExternalUrl(final String url) {
        if (url == null || url.isEmpty()) {
            return;
        }
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                try {
                    Intent intent = new Intent(Intent.ACTION_VIEW, Uri.parse(url));
                    intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
                    startActivity(intent);
                } catch (Exception e) {
                    Log.e(TAG, "Failed to open external URL: " + url, e);
                }
            }
        });
    }

    public void checkForUpdates() {
        Thread worker = new Thread(new Runnable() {
            @Override
            public void run() {
                java.net.HttpURLConnection conn = null;
                try {
                    java.net.URL endpoint = new java.net.URL("https://api.github.com/repos/Masterz42/ErgoNote/releases/latest");
                    conn = (java.net.HttpURLConnection) endpoint.openConnection();
                    conn.setRequestMethod("GET");
                    conn.setRequestProperty("Accept", "application/vnd.github+json");
                    conn.setRequestProperty("User-Agent", "ErgoNote-Android");
                    conn.setConnectTimeout(6000);
                    conn.setReadTimeout(8000);
                    int code = conn.getResponseCode();
                    if (code >= 200 && code < 300) {
                        java.io.InputStream is = conn.getInputStream();
                        java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();
                        byte[] buf = new byte[4096];
                        int total = 0;
                        int n;
                        while ((n = is.read(buf)) != -1 && total < 262144) {
                            out.write(buf, 0, n);
                            total += n;
                        }
                        is.close();
                        nativeOnReleaseJson(out.toString("UTF-8"));
                    }
                } catch (Exception e) {
                    Log.d(TAG, "Update check skipped/offline", e);
                } finally {
                    if (conn != null) {
                        conn.disconnect();
                    }
                }
            }
        }, "ergonote-update-check");
        worker.setDaemon(true);
        worker.start();
    }

    private Uri cameraUri;

    private void openCamera() {
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                try {
                    android.content.ContentValues values = new android.content.ContentValues();
                    values.put(android.provider.MediaStore.Images.Media.DISPLAY_NAME, "ErgoNote-" + System.currentTimeMillis() + ".jpg");
                    values.put(android.provider.MediaStore.Images.Media.MIME_TYPE, "image/jpeg");
                    if (android.os.Build.VERSION.SDK_INT >= 29) {
                        values.put(android.provider.MediaStore.Images.Media.RELATIVE_PATH, "Pictures/ErgoNote");
                    }
                    cameraUri = getContentResolver().insert(android.provider.MediaStore.Images.Media.EXTERNAL_CONTENT_URI, values);
                    if (cameraUri == null) {
                        nativeOnFilePickCancelled();
                        return;
                    }
                    Intent intent = new Intent(android.provider.MediaStore.ACTION_IMAGE_CAPTURE);
                    intent.putExtra(android.provider.MediaStore.EXTRA_OUTPUT, cameraUri);
                    intent.addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION | Intent.FLAG_GRANT_READ_URI_PERMISSION);
                    startActivityForResult(intent, REQ_TAKE_PHOTO);
                } catch (Exception e) {
                    Log.e(TAG, "Failed to launch camera", e);
                    nativeOnFilePickCancelled();
                }
            }
        });
    }

    private void openVoiceDictation(final String locale) {
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                try {
                    Intent intent = new Intent(android.speech.RecognizerIntent.ACTION_RECOGNIZE_SPEECH);
                    intent.putExtra(
                        android.speech.RecognizerIntent.EXTRA_LANGUAGE_MODEL,
                        android.speech.RecognizerIntent.LANGUAGE_MODEL_FREE_FORM
                    );
                    if (locale != null && !locale.isEmpty()) {
                        intent.putExtra(android.speech.RecognizerIntent.EXTRA_LANGUAGE, locale);
                    }
                    intent.putExtra(android.speech.RecognizerIntent.EXTRA_PROMPT, "Dicter votre texte...");
                    startActivityForResult(intent, REQ_VOICE_DICTATION);
                } catch (Exception e) {
                    Log.e(TAG, "Failed to launch Android speech recognition", e);
                    nativeOnVoiceDictated("");
                }
            }
        });
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode == REQ_VOICE_DICTATION) {
            String recognized = "";
            if (resultCode == RESULT_OK && data != null) {
                java.util.ArrayList<String> matches = data.getStringArrayListExtra(android.speech.RecognizerIntent.EXTRA_RESULTS);
                if (matches != null && !matches.isEmpty() && matches.get(0) != null) {
                    recognized = matches.get(0);
                }
            }
            nativeOnVoiceDictated(recognized);
            return;
        }
        if (requestCode == REQ_TAKE_PHOTO) {
            final Uri shot = cameraUri;
            cameraUri = null;
            if (resultCode != RESULT_OK || shot == null) {
                if (shot != null) {
                    try {
                        getContentResolver().delete(shot, null, null);
                    } catch (Exception ignored) {
                    }
                }
                nativeOnFilePickCancelled();
                return;
            }
            new Thread(new Runnable() {
                @Override
                public void run() {
                    copyPickedUriToCache(shot);
                }
            }, "ergonote-photo-copy").start();
            return;
        }
        if (requestCode != REQ_PICK_FILE) {
            return;
        }
        if (resultCode != RESULT_OK || data == null || data.getData() == null) {
            nativeOnFilePickCancelled();
            return;
        }
        final Uri uri = data.getData();
        new Thread(new Runnable() {
            @Override
            public void run() {
                copyPickedUriToCache(uri);
            }
        }, "ergonote-pick-copy").start();
    }

    private void copyPickedUriToCache(Uri uri) {
        try {
            ContentResolver resolver = getContentResolver();
            String displayName = queryDisplayName(resolver, uri);
            if (displayName == null || displayName.trim().isEmpty()) {
                displayName = "fichier";
            }
            if (!displayName.contains(".")) {
                String ext = extensionForMime(resolver.getType(uri));
                if (!ext.isEmpty()) {
                    displayName = displayName + ext;
                }
            }
            String safeName = displayName.replaceAll("[^a-zA-Z0-9._-]", "_");
            if (safeName.isEmpty()) {
                safeName = "fichier.bin";
            }
            File pickedDir = new File(getCacheDir(), "picked");
            if (!pickedDir.exists() && !pickedDir.mkdirs()) {
                Log.e(TAG, "Could not create cache dir: " + pickedDir.getAbsolutePath());
                nativeOnFilePickCancelled();
                return;
            }
            File outFile = new File(pickedDir, System.currentTimeMillis() + "_" + safeName);
            try (InputStream in = resolver.openInputStream(uri);
                 FileOutputStream out = new FileOutputStream(outFile)) {
                if (in == null) {
                    Log.e(TAG, "ContentResolver returned null InputStream for " + uri);
                    nativeOnFilePickCancelled();
                    return;
                }
                byte[] buf = new byte[16384];
                int n;
                while ((n = in.read(buf)) >= 0) {
                    if (n > 0) {
                        out.write(buf, 0, n);
                    }
                }
            }
            nativeOnFilePicked(outFile.getAbsolutePath());
        } catch (Exception e) {
            Log.e(TAG, "Failed to copy picked URI", e);
            nativeOnFilePickCancelled();
        }
    }

    private static String queryDisplayName(ContentResolver resolver, Uri uri) {
        try (Cursor cursor = resolver.query(uri, new String[]{OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (cursor != null && cursor.moveToFirst()) {
                int idx = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME);
                if (idx >= 0) {
                    return cursor.getString(idx);
                }
            }
        } catch (Exception e) {
            Log.w(TAG, "Unable to query display name for " + uri, e);
        }
        String last = uri.getLastPathSegment();
        return last != null ? last : "fichier";
    }

    private static String extensionForMime(String mime) {
        if (mime == null) {
            return "";
        }
        switch (mime) {
            case "image/jpeg":
                return ".jpg";
            case "image/png":
                return ".png";
            case "image/webp":
                return ".webp";
            case "image/gif":
                return ".gif";
            case "image/bmp":
                return ".bmp";
            case "image/svg+xml":
                return ".svg";
            case "application/pdf":
                return ".pdf";
            default:
                return "";
        }
    }

    public void syncImeState(
        final long syncGen,
        final boolean visible,
        final boolean paragraphMode,
        final String text,
        final int selStartUtf16,
        final int selEndUtf16
    ) {
        if (syncGen > latestQueuedSyncGen) {
            latestQueuedSyncGen = syncGen;
        }
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                if (imeView == null || syncGen < latestQueuedSyncGen) {
                    return;
                }
                imeView.syncGen = syncGen;
                InputMethodManager imm = (InputMethodManager) getSystemService(Context.INPUT_METHOD_SERVICE);
                if (visible) {
                    boolean modeChanged = (imeView.paragraphMode != paragraphMode);
                    imeView.paragraphMode = paragraphMode;
                    String safeText = text != null ? text : "";
                    boolean textDiffers = paragraphMode && !safeText.equals(imeView.editable.toString());
                    int clampedStart = Math.max(0, Math.min(selStartUtf16, safeText.length()));
                    int clampedEnd = Math.max(0, Math.min(selEndUtf16, safeText.length()));
                    boolean selDiffers = paragraphMode && (
                        clampedStart != Selection.getSelectionStart(imeView.editable) ||
                        clampedEnd != Selection.getSelectionEnd(imeView.editable)
                    );
                    if (modeChanged || (!paragraphMode && !imeVisible)) {
                        imeView.suppressingCallbacks = true;
                        BaseInputConnection.removeComposingSpans(imeView.editable);
                        imeView.editable.clear();
                        Selection.setSelection(imeView.editable, 0, 0);
                        imeView.lastText = "";
                        imeView.lastSelStart = 0;
                        imeView.lastSelEnd = 0;
                        imeView.suppressingCallbacks = false;
                    }
                    if (paragraphMode && (modeChanged || textDiffers || selDiffers || !imeVisible)) {
                        imeView.suppressingCallbacks = true;
                        BaseInputConnection.removeComposingSpans(imeView.editable);
                        if (textDiffers || modeChanged || !imeVisible) {
                            imeView.editable.replace(0, imeView.editable.length(), safeText);
                        }
                        Selection.setSelection(imeView.editable, clampedStart, clampedEnd);
                        imeView.lastText = safeText;
                        imeView.lastSelStart = clampedStart;
                        imeView.lastSelEnd = clampedEnd;
                        imeView.suppressingCallbacks = false;
                        if (imm != null) {
                            if (textDiffers || modeChanged) {
                                imeView.requestFocus();
                                imm.restartInput(imeView);
                                imm.showSoftInput(imeView, 0);
                            }
                            imm.updateSelection(imeView, clampedStart, clampedEnd, -1, -1);
                        }
                    }
                    if (!imeVisible) {
                        imeVisible = true;
                        imeView.requestFocus();
                        if (imm != null) {
                            imm.restartInput(imeView);
                            imm.showSoftInput(imeView, 0);
                        }
                    }
                } else if (imeVisible) {
                    imeVisible = false;
                    if (imm != null) {
                        imm.hideSoftInputFromWindow(imeView.getWindowToken(), 0);
                    }
                }
            }
        });
    }

    private static final class ImeProxyView extends View {
        final SpannableStringBuilder editable = new SpannableStringBuilder();
        boolean paragraphMode = false;
        boolean suppressingCallbacks = false;
        long syncGen = 0;
        String lastText = "";
        int lastSelStart = 0;
        int lastSelEnd = 0;

        ImeProxyView(Context context) {
            super(context);
            setFocusable(true);
            setFocusableInTouchMode(true);
            setClickable(false);
            setLongClickable(false);
        }

        @Override
        public boolean onTouchEvent(MotionEvent event) {
            return false;
        }

        @Override
        public boolean onCheckIsTextEditor() {
            return true;
        }

        @Override
        public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
            outAttrs.inputType = InputType.TYPE_CLASS_TEXT
                | InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
                | InputType.TYPE_TEXT_FLAG_AUTO_CORRECT
                | InputType.TYPE_TEXT_FLAG_MULTI_LINE;
            outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI
                | EditorInfo.IME_FLAG_NO_FULLSCREEN
                | EditorInfo.IME_ACTION_NONE;
            outAttrs.initialSelStart = Math.max(0, Selection.getSelectionStart(editable));
            outAttrs.initialSelEnd = Math.max(0, Selection.getSelectionEnd(editable));
            return new ErgoInputConnection(this);
        }

        void notifyRustIfChanged() {
            if (suppressingCallbacks) {
                return;
            }
            String current = editable.toString();
            int s = Math.max(0, Selection.getSelectionStart(editable));
            int e = Math.max(0, Selection.getSelectionEnd(editable));
            if (s > current.length()) {
                s = current.length();
            }
            if (e > current.length()) {
                e = current.length();
            }
            if (paragraphMode) {
                if (current.equals(lastText) && s == lastSelStart && e == lastSelEnd) {
                    return;
                }
                lastText = current;
                lastSelStart = s;
                lastSelEnd = e;
                nativeOnImeParagraph(syncGen, current, s, e);
            } else {
                int oldLen = Math.min(lastSelEnd, lastText.length());
                int newLen = Math.min(e, current.length());
                int maxPrefix = Math.min(oldLen, newLen);
                int p = 0;
                while (p < maxPrefix && lastText.charAt(p) == current.charAt(p)) {
                    p++;
                }
                if (p > 0 && (Character.isHighSurrogate(lastText.charAt(p - 1)) || Character.isHighSurrogate(current.charAt(p - 1)))) {
                    p--;
                }
                int deleteBefore = lastText.codePointCount(p, oldLen);
                String insertText = current.substring(p, newLen);
                lastText = current;
                lastSelStart = s;
                lastSelEnd = e;
                if (deleteBefore > 0 || !insertText.isEmpty()) {
                    nativeOnImeDelta(deleteBefore, insertText);
                }
            }
        }
    }

    private static final class ErgoInputConnection extends BaseInputConnection {
        private final ImeProxyView view;
        private int batchDepth = 0;

        ErgoInputConnection(ImeProxyView view) {
            super(view, true);
            this.view = view;
        }

        @Override
        public Editable getEditable() {
            return view.editable;
        }

        @Override
        public boolean beginBatchEdit() {
            batchDepth++;
            return super.beginBatchEdit();
        }

        @Override
        public boolean endBatchEdit() {
            boolean res = super.endBatchEdit();
            if (batchDepth > 0) {
                batchDepth--;
            }
            if (batchDepth == 0) {
                view.notifyRustIfChanged();
            }
            return res;
        }

        private void maybeNotify() {
            if (batchDepth == 0) {
                view.notifyRustIfChanged();
            }
        }

        @Override
        public boolean commitText(CharSequence text, int newCursorPosition) {
            if (view.paragraphMode && text != null) {
                String raw = text.toString();
                int nl = raw.indexOf('\n');
                if (nl >= 0) {
                    String before = raw.substring(0, nl);
                    if (!before.isEmpty()) {
                        super.commitText(before, 1);
                    }
                    super.finishComposingText();
                    view.notifyRustIfChanged();
                    nativeOnImeKey(IME_KEY_ENTER);
                    return true;
                }
            }
            boolean ok = super.commitText(text, newCursorPosition);
            maybeNotify();
            return ok;
        }

        @Override
        public boolean setComposingText(CharSequence text, int newCursorPosition) {
            boolean ok = super.setComposingText(text, newCursorPosition);
            maybeNotify();
            return ok;
        }

        @Override
        public boolean setComposingRegion(int start, int end) {
            boolean ok = super.setComposingRegion(start, end);
            maybeNotify();
            return ok;
        }

        @Override
        public boolean finishComposingText() {
            boolean ok = super.finishComposingText();
            maybeNotify();
            return ok;
        }

        @Override
        public boolean setSelection(int start, int end) {
            boolean ok = super.setSelection(start, end);
            maybeNotify();
            return ok;
        }

        @Override
        public boolean commitCorrection(CorrectionInfo correctionInfo) {
            boolean ok = super.commitCorrection(correctionInfo);
            maybeNotify();
            return ok;
        }

        @Override
        public boolean commitCompletion(CompletionInfo text) {
            boolean ok = super.commitCompletion(text);
            maybeNotify();
            return ok;
        }

        @Override
        public boolean replaceText(int start, int end, CharSequence text, int newCursorPosition, TextAttribute textAttribute) {
            boolean ok = super.replaceText(start, end, text, newCursorPosition, textAttribute);
            maybeNotify();
            return ok;
        }

        @Override
        public boolean deleteSurroundingText(int beforeLength, int afterLength) {
            int selStart = Selection.getSelectionStart(view.editable);
            int selEnd = Selection.getSelectionEnd(view.editable);
            if (beforeLength > 0 && selStart <= 0 && selEnd <= 0) {
                nativeOnImeKey(IME_KEY_BACKSPACE);
                return true;
            }
            boolean ok = super.deleteSurroundingText(beforeLength, afterLength);
            maybeNotify();
            return ok;
        }

        @Override
        public boolean deleteSurroundingTextInCodePoints(int beforeLength, int afterLength) {
            int selStart = Selection.getSelectionStart(view.editable);
            int selEnd = Selection.getSelectionEnd(view.editable);
            if (beforeLength > 0 && selStart <= 0 && selEnd <= 0) {
                nativeOnImeKey(IME_KEY_BACKSPACE);
                return true;
            }
            boolean ok = super.deleteSurroundingTextInCodePoints(beforeLength, afterLength);
            maybeNotify();
            return ok;
        }

        @Override
        public boolean sendKeyEvent(KeyEvent event) {
            if (event != null) {
                int code = event.getKeyCode();
                if (event.getAction() == KeyEvent.ACTION_DOWN) {
                    if (code == KeyEvent.KEYCODE_ENTER) {
                        super.finishComposingText();
                        view.notifyRustIfChanged();
                        nativeOnImeKey(IME_KEY_ENTER);
                        return true;
                    }
                    if (code == KeyEvent.KEYCODE_DEL) {
                        int len = view.editable.length();
                        int selStart = Math.max(0, Math.min(len, Selection.getSelectionStart(view.editable)));
                        int selEnd = Math.max(0, Math.min(len, Selection.getSelectionEnd(view.editable)));
                        if (selStart <= 0 && selEnd <= 0) {
                            nativeOnImeKey(IME_KEY_BACKSPACE);
                            return true;
                        }
                        if (selStart != selEnd) {
                            int lo = Math.min(selStart, selEnd);
                            int hi = Math.max(selStart, selEnd);
                            view.editable.delete(lo, hi);
                        } else {
                            int cp = Character.codePointBefore(view.editable, selStart);
                            int count = Math.min(selStart, Character.charCount(cp));
                            view.editable.delete(selStart - count, selStart);
                        }
                        maybeNotify();
                        return true;
                    }
                    int ch = event.getUnicodeChar();
                    if (ch > 0 && (ch & KeyCharacterMap.COMBINING_ACCENT) == 0) {
                        commitText(new String(Character.toChars(ch)), 1);
                        return true;
                    }
                } else if (event.getAction() == KeyEvent.ACTION_UP) {
                    if (code == KeyEvent.KEYCODE_ENTER || code == KeyEvent.KEYCODE_DEL) {
                        return true;
                    }
                }
            }
            return super.sendKeyEvent(event);
        }
    }
}
