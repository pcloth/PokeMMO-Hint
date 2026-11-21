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
    static final String[] GROUPS = {"弱点", "普通伤害", "抗性", "免疫"};
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
    private long[] observed = new long[0];
    private int width, height, density, rotation, generation;
    private long nextRound;
    private boolean paused, busy, closed;
    private View toolbar, editor, popup;
    private Rect popupBounds;
    private String regionKey;
    private int selectedDetail = -1;
    private boolean toolbarAtBottom;
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
        showToolbar();
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
        return toolbar != null && Rect.intersects(region, viewBounds(toolbar));
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
        Bitmap bitmap = crops.get(at);
        try {
            recognizer.process(InputImage.fromBitmap(bitmap, 0)).addOnCompleteListener(task -> {
                bitmap.recycle();
                if (!closed && token == generation) {
                    int index = indices.get(at);
                    observed[index] = SystemClock.uptimeMillis();
                    Dex.Species match = task.isSuccessful() ? dex.lookup(task.getResult().getText(), false) : null;
                    if (!task.isSuccessful()) {
                        android.util.Log.w("PokemonHint", "OCR failed", task.getException());
                        if (!reportedOcrError) toast("识别失败，请检查游戏文字语言或重启识别");
                        reportedOcrError = true;
                    } else reportedOcrError = false;
                    Dex.Species confirmed = match != null && match == last[index] ? match : null;
                    last[index] = match;
                    if (confirmed != stable[index]) {
                        stable[index] = confirmed;
                        if (confirmed != null) History.record(prefs, confirmed.id());
                        if (selectedDetail == index) closeDetails();
                        renderTags();
                    }
                }
                recognize(crops, indices, at + 1, token);
            });
        } catch (RuntimeException error) {
            android.util.Log.e("PokemonHint", "OCR could not start", error);
            for (int i = at; i < crops.size(); i++) crops.get(i).recycle();
            busy = false;
            paused = true;
            invalidateResults();
            toast("识别引擎启动失败，已暂停");
            showToolbar();
        }
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
        invalidateResults();
        remove(editor); remove(toolbar);
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

    private void showToolbar() {
        remove(toolbar);
        LinearLayout row = new LinearLayout(this);
        row.setBackground(Bubble.panel(0xff183253, dp(12)));
        row.addView(control("＋", "添加选区", () -> editRegion(-1)));
        row.addView(control("☰", "管理选区", this::showRegions));
        row.addView(control(paused ? "▶" : "Ⅱ", paused ? "继续识别" : "暂停识别", () -> {
            paused = !paused; invalidateResults(); showToolbar();
        }));
        row.addView(control("↕", "移动悬浮栏", () -> {
            toolbarAtBottom = !toolbarAtBottom;
            closeDetails();
            showToolbar();
            handler.post(this::renderTags);
        }));
        row.addView(control("×", "结束识别", this::stopSelf));
        toolbar = row;
        int size = dp(42);
        int y = toolbarAtBottom ? Math.max(0, height - size - dp(42)) : dp(44);
        attach(row, params(size * 5, size, Math.max(0, width - size * 5 - dp(8)), y));
    }

    private TextView control(String icon, String description, Runnable action) {
        TextView view = new TextView(this);
        view.setLayoutParams(new LinearLayout.LayoutParams(dp(42), dp(42)));
        view.setGravity(Gravity.CENTER);
        view.setText(icon);
        view.setTextColor(0xffffffff);
        view.setTextSize(21);
        view.setContentDescription(description);
        view.setOnClickListener(v -> action.run());
        return view;
    }

    private void editRegion(int index) {
        if (index < 0 && areas.size() >= Regions.LIMIT) { toast("最多保存 16 个选区"); return; }
        remove(editor);
        invalidateResults();
        toolbar.setVisibility(View.GONE);
        editor = new RegionEditor(this, width, height, index < 0 ? null : areas.get(index), (name, rect) -> {
            remove(editor); editor = null;
            toolbar.setVisibility(View.VISIBLE);
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
        for (int i = 0; i < stable.length; i++) {
            if (stable[i] == null) continue;
            int index = i;
            Rect region = areas.get(i).rect();
            TextView label = new TextView(this);
            label.setText(dex.label(stable[i]));
            label.setTextColor(0xff9e2830);
            label.setTextSize(14);
            label.setGravity(Gravity.CENTER);
            label.setPadding(dp(11), 0, dp(11), 0);
            label.measure(View.MeasureSpec.UNSPECIFIED, View.MeasureSpec.UNSPECIFIED);
            int w = Math.max(dp(65), label.getMeasuredWidth()), h = dp(38);
            Rect location = place(region, w, h, true);
            if (location == null) continue;
            Bubble bubble = new Bubble(this, location, region, 0xffffeef0, 0xfff1bcc2, dp(8));
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
        LinearLayout rows = new LinearLayout(this);
        rows.setOrientation(LinearLayout.VERTICAL);
        rows.setPadding(dp(14), dp(8), dp(14), dp(8));
        TextView title = new TextView(this);
        title.setText(dex.title(entry, language));
        title.setTextColor(0xff172a44);
        title.setTextSize(19);
        title.setTypeface(null, Typeface.BOLD);
        rows.addView(title);
        String[] details = dex.details(entry);
        int[] colors = {0xffa52b35, 0xff4c5767, 0xff146449, 0xff693ca1};
        for (int i = 0; i < GROUPS.length; i++) {
            TextView line = new TextView(this);
            line.setText(GROUPS[i] + "  " + details[i]);
            line.setTextSize(14);
            line.setTextColor(colors[i]);
            line.setPadding(0, dp(7), 0, dp(3));
            rows.addView(line);
        }
        Button close = new Button(this);
        close.setText("关闭详情");
        close.setOnClickListener(v -> closeDetails());
        rows.addView(close);
        int w = Math.min(width - dp(12), dp(345));
        rows.measure(View.MeasureSpec.makeMeasureSpec(w - dp(8), View.MeasureSpec.EXACTLY), View.MeasureSpec.UNSPECIFIED);
        int h = Math.min(height - dp(62), rows.getMeasuredHeight() + dp(16));
        Rect location = null;
        while (h >= dp(95) && (location = place(region, w, h, false)) == null) h -= dp(24);
        if (location == null) { toast("选区旁没有足够空间展示详情"); return; }
        Bubble bubble = new Bubble(this, location, region, 0xfffbfdff, 0xffa3bbd5, dp(8));
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
