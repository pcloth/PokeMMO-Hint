package local.pokemon.hint;

import android.app.*;
import android.content.*;
import android.content.res.Configuration;
import android.graphics.*;
import android.hardware.display.*;
import android.media.*;
import android.media.projection.*;
import android.os.*;
import android.provider.Settings;
import android.util.DisplayMetrics;
import android.view.*;
import android.widget.*;

import com.google.mlkit.vision.common.InputImage;
import com.google.mlkit.vision.text.TextRecognition;
import com.google.mlkit.vision.text.TextRecognizer;
import com.google.mlkit.vision.text.chinese.ChineseTextRecognizerOptions;
import com.google.mlkit.vision.text.japanese.JapaneseTextRecognizerOptions;
import com.google.mlkit.vision.text.latin.TextRecognizerOptions;
import java.util.*;

public final class CaptureService extends Service {
    static boolean active;
    private static final String STOP = "local.pokemon.hint.STOP";
    private static final String CHANNEL = "screen_capture";
    private final Handler handler = new Handler(Looper.getMainLooper());
    private final List<View> tags = new ArrayList<>();
    private final List<Rect> tagBounds = new ArrayList<>();
    private WindowManager windows;
    private SharedPreferences prefs;
    private MediaProjection projection;
    private VirtualDisplay display;
    private ImageReader reader;
    private TextRecognizer recognizer;
    private Dex dex;
    private Dex.Language language;
    private List<Regions.Area> areas = new ArrayList<>();
    private Dex.Species[] last = new Dex.Species[0];
    private Dex.Species[] stable = new Dex.Species[0];
    private static final long MISSED_TEXT_GRACE_MS = 2500;
    private long[] observed = new long[0];
    private long[] missingSince = new long[0];
    private int width, height, density, rotation, generation;
    private long nextRound;
    private boolean paused, busy, closed;
    private FloatingControls controls;
    private View editor, popup;
    private final Runnable refreshTags = this::renderTags;
    private final SharedPreferences.OnSharedPreferenceChangeListener tagSettings = (settings, key) -> {
        if (MainActivity.SHOW_ALL_WEAKNESSES.equals(key)) renderTags();
    };
    private Rect popupBounds;
    private String regionKey;
    private int selectedDetail = -1;
    private boolean reportedOcrError;

    @Override public IBinder onBind(Intent intent) { return null; }

    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent == null || STOP.equals(intent.getAction())) { stopSelf(); return START_NOT_STICKY; }
        if (active) return START_NOT_STICKY;
        try {
            startForeground(1, notification());
            Intent consent = Build.VERSION.SDK_INT >= 33
                    ? intent.getParcelableExtra("projection", Intent.class) : intent.getParcelableExtra("projection");
            if (consent == null || !Settings.canDrawOverlays(this)) throw new IllegalStateException();
            windows = getSystemService(WindowManager.class);
            prefs = getSharedPreferences(MainActivity.PREFS, MODE_PRIVATE);
            language = Dex.Language.values()[Math.max(0, Math.min(2, prefs.getInt("language", 0)))];
            var assets = getAssets();
            dex = new Dex(assets.open("species.tsv"), assets.open("species_names.tsv"), assets.open("type_chart.txt"));
            recognizer = switch (language) {
                case ZH -> TextRecognition.getClient(new ChineseTextRecognizerOptions.Builder().build());
                case EN -> TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS);
                case JA -> TextRecognition.getClient(new JapaneseTextRecognizerOptions.Builder().build());
            };
            projection = getSystemService(MediaProjectionManager.class).getMediaProjection(Activity.RESULT_OK, consent);
            projection.registerCallback(new MediaProjection.Callback() {
                @Override public void onStop() { stopSelf(); }
                @Override public void onCapturedContentResize(int w, int h) {
                    if (!closed) refreshDisplay(w, h);
                }
            }, handler);
            updateDisplay(0, 0);
            prefs.registerOnSharedPreferenceChangeListener(tagSettings);
            active = true;
        } catch (Exception error) {
            android.util.Log.e("PokemonHint", "Unable to start capture", error);
            toast("无法开启识别，请重新授权录屏和悬浮窗");
            stopSelf();
        }
        return START_NOT_STICKY;
    }

    private Notification notification() {
        var manager = getSystemService(NotificationManager.class);
        manager.createNotificationChannel(new NotificationChannel(CHANNEL, "屏幕识别", NotificationManager.IMPORTANCE_LOW));
        int flags = PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT;
        var stop = PendingIntent.getService(this, 1, new Intent(this, CaptureService.class).setAction(STOP), flags);
        var open = PendingIntent.getActivity(this, 2, new Intent(this, MainActivity.class), flags);
        return new Notification.Builder(this, CHANNEL).setSmallIcon(android.R.drawable.ic_menu_search)
                .setContentTitle("宝可梦属性提示").setContentText("正在读取屏幕选区")
                .setContentIntent(open).addAction(new Notification.Action.Builder(null, "结束识别", stop).build())
                .setOngoing(true).build();
    }

    @Override public void onConfigurationChanged(Configuration configuration) {
        super.onConfigurationChanged(configuration);
        if (active && !closed) refreshDisplay(0, 0);
    }

    private void refreshDisplay(int w, int h) {
        try { updateDisplay(w, h); }
        catch (RuntimeException error) {
            android.util.Log.w("PokemonHint", "Could not resize capture", error);
            toast("屏幕尺寸变化，请重新授权录屏");
            stopSelf();
        }
    }

    private void updateDisplay(int captureWidth, int captureHeight) {
        DisplayMetrics metrics = new DisplayMetrics();
        windows.getDefaultDisplay().getRealMetrics(metrics);
        int currentRotation = windows.getDefaultDisplay().getRotation();
        if (captureWidth > 0 && (captureWidth != metrics.widthPixels || captureHeight != metrics.heightPixels)) {
            // A single-app capture or an OEM crop has no reliable display-space offset.
            toast("请重新开启识别并共享整个屏幕");
            stopSelf();
            return;
        }
        if (reader != null && width == metrics.widthPixels && height == metrics.heightPixels && rotation == currentRotation) return;
        remove(editor); editor = null;
        invalidateResults();
        width = metrics.widthPixels;
        height = metrics.heightPixels;
        density = metrics.densityDpi;
        rotation = currentRotation;
        regionKey = Regions.key(width, height, rotation);
        areas = Regions.load(prefs, regionKey, width, height);
        last = new Dex.Species[areas.size()];
        stable = new Dex.Species[areas.size()];
        observed = new long[areas.size()];
        missingSince = new long[areas.size()];
        ImageReader next = ImageReader.newInstance(width, height, PixelFormat.RGBA_8888, 2);
        ImageReader old = reader;
        reader = next;
        try {
            if (display == null) {
                display = projection.createVirtualDisplay("PokemonHint", width, height, density,
                        DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR, next.getSurface(), null, handler);
            } else {
                display.resize(width, height, density);
                display.setSurface(next.getSurface());
            }
        } finally {
            if (old != null) old.close();
        }
        next.setOnImageAvailableListener(this::onFrame, handler);
        showControls();
    }

    private void onFrame(ImageReader source) {
        if (source != reader || closed) return;
        List<Bitmap> crops = null;
        List<Integer> indices;
        try (Image image = source.acquireLatestImage()) {
            if (image == null || paused || editor != null || busy || areas.isEmpty()
                    || SystemClock.uptimeMillis() < nextRound) return;
            long now = SystemClock.uptimeMillis();
            expireResults(now);
            crops = new ArrayList<>();
            indices = new ArrayList<>();
            nextRound = now + MainActivity.INTERVALS[Math.max(0, Math.min(3, prefs.getInt("rate", 1)))];
            for (int i = 0; i < areas.size(); i++) {
                Rect region = areas.get(i).rect();
                if ((popupBounds != null && Rect.intersects(region, popupBounds)) || intersectsToolbar(region)) continue;
                crops.add(crop(image, region));
                indices.add(i);
            }
        } catch (RuntimeException error) {
            if (crops != null) for (Bitmap crop : crops) crop.recycle();
            return;
        }
        if (crops.isEmpty()) return;
        busy = true;
        recognize(crops, indices, 0, generation);
    }

    private boolean intersectsToolbar(Rect region) {
        if (controls == null || controls.getVisibility() != View.VISIBLE) return false;
        var position = controls.position();
        Rect destination = new Rect(position.x(), position.y(),
                position.x() + position.width(), position.y() + position.height());
        return Rect.intersects(region, destination) || Rect.intersects(region, viewBounds(controls));
    }

    private Bitmap crop(Image image, Rect rect) {
        var plane = image.getPlanes()[0];
        var buffer = plane.getBuffer();
        int width = rect.width(), height = rect.height();
        int[] pixels = new int[width * height];
        int origin = buffer.position();
        for (int y = 0; y < height; y++) {
            int row = origin + (rect.top + y) * plane.getRowStride() + rect.left * plane.getPixelStride();
            for (int x = 0; x < width; x++) {
                int index = row + x * plane.getPixelStride();
                pixels[y * width + x] = Color.rgb(buffer.get(index) & 255, buffer.get(index + 1) & 255, buffer.get(index + 2) & 255);
            }
        }
        Bitmap original = Bitmap.createBitmap(pixels, width, height, Bitmap.Config.ARGB_8888);
        float scale = Math.min(3f, 1600f / Math.max(width, height));
        Bitmap scaled = Bitmap.createScaledBitmap(original, Math.max(1, Math.round(width * scale)),
                Math.max(1, Math.round(height * scale)), true);
        if (scaled != original) original.recycle();
        return scaled;
    }

    private void recognize(List<Bitmap> crops, List<Integer> indices, int at, int token) {
        if (closed || token != generation || at == crops.size()) {
            for (int i = at; i < crops.size(); i++) crops.get(i).recycle();
            busy = false;
            return;
        }
        Bitmap original = crops.get(at);
        readText(original, (match, retry) -> {
            Bitmap filtered = null;
            if (retry && !closed && token == generation) {
                filtered = OcrImage.whiteText(original);
            }
            original.recycle();
            if (filtered == null) {
                finishRead(crops, indices, at, token, match);
                return;
            }
            Bitmap fallback = filtered;
            readText(fallback, (secondMatch, ignored) -> {
                fallback.recycle();
                finishRead(crops, indices, at, token, secondMatch);
            });
        });
    }

    private void readText(Bitmap bitmap, java.util.function.BiConsumer<Dex.Species, Boolean> result) {
        try {
            recognizer.process(InputImage.fromBitmap(bitmap, 0)).addOnCompleteListener(task -> {
                if (closed) {
                    result.accept(null, false);
                } else if (task.isSuccessful()) {
                    reportedOcrError = false;
                    Dex.Species match = dex.lookup(task.getResult().getText(), false);
                    result.accept(match, match == null);
                } else {
                    reportOcrError(task.getException());
                    result.accept(null, false);
                }
            });
        } catch (RuntimeException error) {
            reportOcrError(error);
            result.accept(null, false);
        }
    }

    private void reportOcrError(Exception error) {
        android.util.Log.w("PokemonHint", "OCR failed", error);
        if (!reportedOcrError) toast("识别失败，请检查游戏文字语言或重启识别");
        reportedOcrError = true;
    }

    private void finishRead(List<Bitmap> crops, List<Integer> indices, int at, int token, Dex.Species match) {
        if (!closed && token == generation) {
            int index = indices.get(at);
            long now = SystemClock.uptimeMillis();
            observed[index] = now;
            Dex.Species previous = stable[index];
            Dex.Species confirmed;
            if (match == null) {
                if (missingSince[index] == 0) missingSince[index] = now;
                confirmed = now - missingSince[index] < MISSED_TEXT_GRACE_MS ? previous : null;
            } else {
                missingSince[index] = 0;
                confirmed = match == previous || match == last[index] ? match : null;
            }
            last[index] = match;
            if (confirmed != previous) {
                stable[index] = confirmed;
                if (confirmed != null) History.record(prefs, confirmed.id());
                if (selectedDetail == index) closeDetails();
                renderTags();
            }
        }
        // Post so a synchronous recognizer failure cannot recurse through the batch.
        handler.post(() -> recognize(crops, indices, at + 1, token));
    }

    private void expireResults(long now) {
        boolean changed = false;
        for (int i = 0; i < observed.length; i++) {
            if (now - observed[i] <= 15_000) continue;
            last[i] = null;
            if (stable[i] != null) {
                stable[i] = null;
                changed = true;
                if (selectedDetail == i) closeDetails();
            }
        }
        if (changed) renderTags();
    }

    private void invalidateResults() {
        generation++;
        Arrays.fill(last, null);
        Arrays.fill(stable, null);
        Arrays.fill(missingSince, 0);
        clearTags();
        closeDetails();
        nextRound = 0;
    }

    private void clearTags() {
        for (View view : tags) remove(view);
        tags.clear();
        tagBounds.clear();
    }

    private void closeDetails() {
        remove(popup); popup = null; popupBounds = null; selectedDetail = -1;
    }

    @Override public void onDestroy() {
        closed = true;
        active = false;
        if (prefs != null) prefs.unregisterOnSharedPreferenceChangeListener(tagSettings);
        invalidateResults();
        handler.removeCallbacks(refreshTags);
        remove(editor); remove(controls);
        controls = null;
        if (display != null) display.release();
        if (reader != null) reader.close();
        if (projection != null) projection.stop();
        if (recognizer != null) recognizer.close();
        stopForeground(STOP_FOREGROUND_REMOVE);
        super.onDestroy();
    }

    private int dp(int value) { return Math.max(1, Math.round(value * density / 160f)); }
    private void toast(String message) { Toast.makeText(this, message, Toast.LENGTH_SHORT).show(); }

    private WindowManager.LayoutParams params(int w, int h, int x, int y) {
        var p = new WindowManager.LayoutParams(w, h, WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE | WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN,
                PixelFormat.TRANSLUCENT);
        p.gravity = Gravity.TOP | Gravity.LEFT;
        p.x = x; p.y = y;
        if (Build.VERSION.SDK_INT >= 30) {
            p.setFitInsetsTypes(0);
            p.layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS;
        } else if (Build.VERSION.SDK_INT >= 28) {
            p.layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
        }
        return p;
    }

    private boolean attach(View view, WindowManager.LayoutParams p) {
        try { windows.addView(view, p); return true; }
        catch (RuntimeException error) {
            android.util.Log.w("PokemonHint", "Overlay unavailable", error);
            toast("悬浮窗不可用，请检查系统权限");
            stopSelf();
            return false;
        }
    }

    private void remove(View view) {
        if (view == null) return;
        try { windows.removeViewImmediate(view); }
        catch (IllegalArgumentException ignored) { /* Already removed by WindowManager. */ }
    }

    private Rect viewBounds(View view) {
        int[] xy = new int[2];
        view.getLocationOnScreen(xy);
        return new Rect(xy[0], xy[1], xy[0] + view.getWidth(), xy[1] + view.getHeight());
    }

    private void showControls() {
        remove(controls);
        controls = null;
        Rect area = new Rect(0, 0, width, height);
        if (Build.VERSION.SDK_INT >= 30) {
            var insets = windows.getMaximumWindowMetrics().getWindowInsets()
                    .getInsetsIgnoringVisibility(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
            area.set(insets.left, insets.top, width - insets.right, height - insets.bottom);
        } else {
            // Older overlay windows use the platform's status/navigation bar bounds.
            area.top = dp(26);
            area.bottom = Math.max(area.top + 1, height - dp(48));
        }
        controls = new FloatingControls(this, area, handler, this::repositionControls,
                () -> editRegion(-1), this::showRegions, () -> {
                    paused = !paused;
                    invalidateResults();
                    controls.setPaused(paused);
                }, this::openMainWindow, this::stopSelf);
        controls.setPaused(paused);
        var position = controls.position();
        attach(controls, params(position.width(), position.height(), position.x(), position.y()));
    }

    private void openMainWindow() {
        closeDetails();
        startActivity(new Intent(this, MainActivity.class).addFlags(
                Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP | Intent.FLAG_ACTIVITY_SINGLE_TOP));
    }

    private void repositionControls() {
        if (controls == null || closed || !controls.isAttachedToWindow()) return;
        var position = controls.position();
        var layout = (WindowManager.LayoutParams) controls.getLayoutParams();
        layout.width = position.width();
        layout.height = position.height();
        layout.x = position.x();
        layout.y = position.y();
        windows.updateViewLayout(controls, layout);
        handler.removeCallbacks(refreshTags);
        if (!controls.isDragging()) handler.postDelayed(refreshTags, 50);
    }

    private void editRegion(int index) {
        if (index < 0 && areas.size() >= Regions.LIMIT) { toast("最多保存 16 个选区"); return; }
        remove(editor);
        invalidateResults();
        controls.collapse();
        controls.setVisibility(View.GONE);
        editor = new RegionEditor(this, width, height, index < 0 ? null : areas.get(index), (name, rect) -> {
            remove(editor); editor = null;
            controls.setVisibility(View.VISIBLE);
            if (rect != null) {
                Regions.Area area = new Regions.Area(name.isEmpty() ? "选区 " + (index < 0 ? areas.size() + 1 : index + 1) : name, rect);
                if (index < 0) areas.add(area); else areas.set(index, area);
                saveRegions();
            }
            nextRound = 0;
        });
        var layout = params(-1, -1, 0, 0);
        layout.flags |= WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM;
        attach(editor, layout);
    }

    private void saveRegions() {
        Regions.save(prefs, regionKey, areas);
        invalidateResults();
        last = new Dex.Species[areas.size()];
        stable = new Dex.Species[areas.size()];
        observed = new long[areas.size()];
        missingSince = new long[areas.size()];
    }

    private void showRegions() {
        if (areas.isEmpty()) { toast("请先添加选区"); return; }
        if (popup != null && selectedDetail == -1) { closeDetails(); return; }
        closeDetails();
        LinearLayout rows = new LinearLayout(this);
        rows.setOrientation(LinearLayout.VERTICAL);
        rows.setPadding(dp(10), dp(8), dp(10), dp(8));
        rows.setBackground(Bubble.panel(0xfff2f6fb, dp(14)));
        for (int i = 0; i < areas.size(); i++) {
            int index = i;
            LinearLayout row = new LinearLayout(this);
            Button adjust = new Button(this);
            adjust.setText(areas.get(i).name() + " · 调整");
            adjust.setOnClickListener(v -> editRegion(index));
            row.addView(adjust, new LinearLayout.LayoutParams(0, -2, 1));
            Button delete = new Button(this);
            delete.setText("删除");
            delete.setOnClickListener(v -> { areas.remove(index); saveRegions(); });
            row.addView(delete);
            rows.addView(row);
        }
        Button close = new Button(this);
        close.setText("关闭");
        close.setOnClickListener(v -> closeDetails());
        rows.addView(close);
        ScrollView scroll = new ScrollView(this);
        scroll.addView(rows);
        int w = Math.min(width - dp(16), dp(320)), h = Math.min(height - dp(130), dp(54 * (areas.size() + 1) + 16));
        if (w > 0 && h > 0) showPopup(scroll, new Rect(dp(8), dp(92), dp(8) + w, dp(92) + h));
    }

    private void renderTags() {
        if (closed || editor != null) return;
        clearTags();
        boolean showAll = prefs.getBoolean(MainActivity.SHOW_ALL_WEAKNESSES, false);
        int tip = dp(8);
        for (int i = 0; i < stable.length; i++) {
            if (stable[i] == null) continue;
            int index = i;
            Rect region = areas.get(i).rect();
            TextView label = new TextView(this);
            label.setText(dex.label(stable[i], showAll));
            label.setTextColor(0xff9e2830);
            label.setTextSize(14);
            label.setGravity(Gravity.CENTER);
            label.setPadding(dp(11), 0, dp(11), 0);
            label.measure(View.MeasureSpec.UNSPECIFIED, View.MeasureSpec.UNSPECIFIED);
            int w = Math.min(width - dp(8), Math.max(dp(65), label.getMeasuredWidth() + tip));
            if (w <= tip) continue;
            // Reserve space for a side-facing tip as well as a top/bottom tip.
            label.measure(View.MeasureSpec.makeMeasureSpec(w - tip, View.MeasureSpec.EXACTLY),
                    View.MeasureSpec.UNSPECIFIED);
            int h = Math.max(dp(38), label.getMeasuredHeight() + dp(8) + tip);
            if (h > height) continue;
            Rect location = place(region, w, h, true);
            if (location == null) continue;
            Bubble bubble = new Bubble(this, location, region, 0xffffeef0, 0xfff1bcc2, tip);
            bubble.addView(label, new FrameLayout.LayoutParams(-1, -1));
            label.setOnClickListener(v -> showDetails(index));
            tags.add(bubble);
            tagBounds.add(location);
            attach(bubble, params(w, h, location.left, location.top));
        }
    }

    private Rect place(Rect region, int w, int h, boolean tag) {
        int gap = dp(4);
        int[][] order = {{region.left, region.top - h - gap}, {region.left, region.bottom + gap},
                {region.right + gap, region.top}, {region.left - w - gap, region.top}};
        int[] preference = tag ? new int[]{0, 1, 2, 3} : new int[]{1, 2, 3, 0};
        Rect best = null;
        int bestScore = Integer.MAX_VALUE;
        for (int candidate : preference) {
            int x = Math.max(0, Math.min(width - w, order[candidate][0]));
            int y = Math.max(0, Math.min(height - h, order[candidate][1]));
            Rect box = new Rect(x, y, x + w, y + h);
            if (intersectsToolbar(box) || Rect.intersects(box, region)) continue;
            int score = 0;
            for (Regions.Area area : areas) if (Rect.intersects(box, area.rect())) score += 4;
            for (Rect shown : tagBounds) if (Rect.intersects(box, shown)) score++;
            if (score < bestScore) { best = box; bestScore = score; }
        }
        return tag && bestScore >= 4 ? null : best;
    }

    private void showDetails(int index) {
        if (selectedDetail == index) { closeDetails(); return; }
        closeDetails();
        Dex.Species entry = stable[index];
        if (entry == null) return;
        Rect region = areas.get(index).rect();
        View rows = HintUi.card(this, dex, entry, language, this::closeDetails);
        int w = Math.min(width - dp(12), dp(345));
        rows.measure(View.MeasureSpec.makeMeasureSpec(w - dp(8), View.MeasureSpec.EXACTLY), View.MeasureSpec.UNSPECIFIED);
        int h = Math.min(height - dp(62), rows.getMeasuredHeight() + dp(16));
        Rect location = null;
        while (h >= dp(95) && (location = place(region, w, h, false)) == null) h -= dp(24);
        if (location == null) { toast("选区旁没有足够空间展示详情"); return; }
        Bubble bubble = new Bubble(this, location, region, HintUi.WHITE, HintUi.BORDER, dp(8));
        ScrollView scroll = new ScrollView(this);
        scroll.addView(rows);
        bubble.addView(scroll, new FrameLayout.LayoutParams(-1, -1));
        selectedDetail = index;
        showPopup(bubble, location);
    }

    private void showPopup(View view, Rect rect) {
        popup = view; popupBounds = rect;
        attach(view, params(rect.width(), rect.height(), rect.left, rect.top));
    }
}
