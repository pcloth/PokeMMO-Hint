package local.pokemon.hint;

import android.app.*;
import android.content.*;
import android.media.projection.*;
import android.net.Uri;
import android.os.*;
import android.provider.Settings;
import android.view.*;
import android.widget.*;

public final class MainActivity extends Activity {
    static final String PREFS = "settings";
    static final String SHOW_ALL_WEAKNESSES = "show_all_weaknesses";
    static final int[] INTERVALS = {2000, 1000, 500, 250};
    private static final int CAPTURE = 12;
    private TextView status, captureState;
    private LinearLayout result, history;
    private SharedPreferences prefs;
    private ScrollView scroll;
    private Dex dex;
    private Dex.Species current;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        prefs = getSharedPreferences(PREFS, MODE_PRIVATE);
        scroll = new ScrollView(this);
        scroll.setFillViewport(true);
        scroll.setBackgroundColor(HintUi.PAGE);
        LinearLayout content = HintUi.column(this);
        content.setPadding(dp(16), dp(14), dp(16), dp(20));
        scroll.addView(content);
        if (Build.VERSION.SDK_INT >= 35) {
            scroll.setOnApplyWindowInsetsListener((view, insets) -> {
                var bars = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
                view.setPadding(bars.left, bars.top, bars.right, bars.bottom);
                return insets;
            });
        }
        setContentView(scroll);
        LinearLayout header = new LinearLayout(this);
        header.setGravity(Gravity.CENTER_VERTICAL);
        header.addView(HintUi.text(this, "属性速查", 27, HintUi.INK, true),
                new LinearLayout.LayoutParams(0, -2, 1));
        captureState = HintUi.text(this, "", 13, HintUi.MUTED, false);
        header.addView(captureState);
        content.addView(header);

        LinearLayout toolbar = new LinearLayout(this);
        addTool(toolbar, R.drawable.tool_play, "开启悬浮识别", true, this::requestCapture);
        addTool(toolbar, R.drawable.tool_cancel, "结束识别", false, () -> {
            stopService(new Intent(this, CaptureService.class));
            captureState.setText("已暂停");
            status.setText("已结束识别");
        });
        addTool(toolbar, R.drawable.tool_settings, "识别设置", false, this::showSettings);
        addSection(content, toolbar, 8);
        status = HintUi.text(this, "", 13, HintUi.MUTED, false);
        status.setPadding(0, dp(8), 0, dp(10));
        content.addView(status);

        boolean wide = getResources().getConfiguration().screenWidthDp >= 680;
        LinearLayout body = new LinearLayout(this);
        body.setOrientation(wide ? LinearLayout.HORIZONTAL : LinearLayout.VERTICAL);
        content.addView(body);
        LinearLayout main = HintUi.column(this);
        body.addView(main, wide ? new LinearLayout.LayoutParams(0, -2, 2) : new LinearLayout.LayoutParams(-1, -2));

        result = HintUi.column(this);
        result.setMinimumHeight(dp(168));
        result.setBackground(HintUi.panel(this, HintUi.WHITE, 16));
        main.addView(result, new LinearLayout.LayoutParams(-1, -2));
        resultMessage("准备好后，开始识别", "返回游戏后点开悬浮球，框选一行名字或 No. 编号。");

        addSection(main, HintUi.text(this, "手动查询", 13, HintUi.MUTED, false), 14);
        LinearLayout search = new LinearLayout(this);
        search.setGravity(Gravity.CENTER_VERTICAL);
        EditText query = new EditText(this);
        query.setSingleLine();
        query.setHint("名字或全国编号");
        query.setTextSize(16);
        query.setPadding(dp(12), 0, dp(12), 0);
        query.setBackground(HintUi.panel(this, HintUi.WHITE, 8));
        query.setImeOptions(android.view.inputmethod.EditorInfo.IME_ACTION_SEARCH);
        search.addView(query, new LinearLayout.LayoutParams(0, dp(48), 1));
        var find = HintUi.icon(this, R.drawable.tool_search, "查询", false, () -> lookup(query));
        var findLayout = new LinearLayout.LayoutParams(dp(48), dp(48));
        findLayout.leftMargin = dp(8);
        search.addView(find, findLayout);
        query.setOnEditorActionListener((view, action, event) -> {
            if (action != android.view.inputmethod.EditorInfo.IME_ACTION_SEARCH) return false;
            lookup(query);
            return true;
        });
        addSection(main, search, 5);

        history = HintUi.column(this);
        history.setPadding(dp(14), dp(12), dp(14), dp(12));
        history.setBackground(HintUi.panel(this, HintUi.WHITE, 16));
        if (wide) {
            var layout = new LinearLayout.LayoutParams(0, -2, 1);
            layout.leftMargin = dp(12);
            body.addView(history, layout);
        } else addSection(body, history, 14);
        try {
            var assets = getAssets();
            dex = new Dex(assets.open("species.tsv"), assets.open("species_names.tsv"), assets.open("type_chart.txt"));
        } catch (Exception error) {
            resultMessage("无法加载本地图鉴", "");
        }
    }

    @Override protected void onResume() {
        super.onResume();
        captureState.setText(CaptureService.active ? "识别服务运行中" : "已暂停");
        renderHistory();
    }

    private void addTool(LinearLayout toolbar, int resource, String label, boolean primary, Runnable action) {
        var layout = new LinearLayout.LayoutParams(dp(48), dp(48));
        layout.rightMargin = dp(8);
        toolbar.addView(HintUi.icon(this, resource, label, primary, action), layout);
    }

    private void addSection(LinearLayout parent, View child, int margin) {
        var layout = new LinearLayout.LayoutParams(-1, -2);
        layout.topMargin = dp(margin);
        parent.addView(child, layout);
    }

    private Dex.Language language() {
        return Dex.Language.values()[Math.max(0, Math.min(2, prefs.getInt("language", 0)))];
    }

    private void lookup(EditText query) {
        if (dex == null) return;
        display(dex.lookup(query.getText().toString(), true));
        getSystemService(android.view.inputmethod.InputMethodManager.class)
                .hideSoftInputFromWindow(query.getWindowToken(), 0);
        query.clearFocus();
    }

    private void display(Dex.Species entry) {
        current = entry;
        if (entry == null) {
            resultMessage("未匹配到物种", "试试完整名字或全国编号。");
            return;
        }
        result.removeAllViews();
        result.addView(HintUi.card(this, dex, entry, language(), null));
    }

    private void resultMessage(String title, String hint) {
        result.removeAllViews();
        LinearLayout empty = HintUi.column(this);
        empty.setPadding(dp(14), dp(24), dp(14), dp(24));
        empty.setGravity(Gravity.CENTER);
        TextView heading = HintUi.text(this, title, 19, HintUi.INK, true);
        heading.setGravity(Gravity.CENTER);
        empty.addView(heading);
        if (!hint.isEmpty()) {
            TextView copy = HintUi.text(this, hint, 14, HintUi.MUTED, false);
            copy.setGravity(Gravity.CENTER);
            addSection(empty, copy, 8);
        }
        result.addView(empty);
    }

    private void renderHistory() {
        history.removeAllViews();
        history.addView(HintUi.text(this, "识别历史", 17, HintUi.INK, true));
        var ids = History.read(prefs);
        if (dex == null || ids.isEmpty()) {
            addSection(history, HintUi.text(this, "暂无识别记录", 14, HintUi.MUTED, false), 8);
            return;
        }
        for (int id : ids) {
            Dex.Species entry = dex.lookup(Integer.toString(id), true);
            if (entry == null) continue;
            TextView item = HintUi.text(this, dex.title(entry, language()), 14, HintUi.INK, false);
            item.setGravity(Gravity.CENTER_VERTICAL);
            item.setMinHeight(dp(44));
            item.setPadding(dp(10), dp(5), dp(10), dp(5));
            item.setBackground(HintUi.panel(this, HintUi.PAGE, 8));
            item.setFocusable(true);
            item.setOnClickListener(view -> {
                display(entry);
                scroll.smoothScrollTo(0, 0);
            });
            addSection(history, item, 6);
        }
    }

    private void showSettings() {
        LinearLayout settings = HintUi.column(this);
        settings.setPadding(dp(22), dp(8), dp(22), dp(8));
        settings.addView(HintUi.text(this, "游戏文字语言", 15, HintUi.INK, true));
        Spinner languages = spinner(new String[]{"简体中文", "English", "日本語"}, language().ordinal());
        addSection(settings, languages, 4);
        addSection(settings, HintUi.text(this, "识别刷新率", 15, HintUi.INK, true), 12);
        Spinner rates = spinner(new String[]{"0.5 次/秒", "1 次/秒", "2 次/秒", "4 次/秒"}, prefs.getInt("rate", 1));
        addSection(settings, rates, 4);
        CheckBox allWeaknesses = new CheckBox(this);
        allWeaknesses.setText("显示全部弱点（合并为一个标签）");
        allWeaknesses.setChecked(prefs.getBoolean(SHOW_ALL_WEAKNESSES, false));
        addSection(settings, allWeaknesses, 8);

        ScrollView settingsScroll = new ScrollView(this);
        settingsScroll.addView(settings);
        new AlertDialog.Builder(this).setTitle("识别设置").setView(settingsScroll).setNegativeButton("取消", null)
                .setPositiveButton("应用", (dialog, button) -> {
                    int newLanguage = languages.getSelectedItemPosition();
                    boolean changed = newLanguage != language().ordinal();
                    prefs.edit().putInt("language", newLanguage)
                            .putInt("rate", rates.getSelectedItemPosition())
                            .putBoolean(SHOW_ALL_WEAKNESSES, allWeaknesses.isChecked()).apply();
                    status.setText(changed && CaptureService.active ? "重新开启识别后会切换语言。" : "识别设置已保存");
                    if (current != null) display(current);
                    renderHistory();
                }).show();
    }

    private void requestCapture() {
        if (CaptureService.active) {
            moveTaskToBack(true);
            return;
        }
        if (!Settings.canDrawOverlays(this)) {
            status.setText("请允许显示在其他应用上层，返回后再次开启。");
            startActivity(new Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:" + getPackageName())));
            return;
        }
        var manager = getSystemService(MediaProjectionManager.class);
        var request = Build.VERSION.SDK_INT >= 34
                ? manager.createScreenCaptureIntent(MediaProjectionConfig.createConfigForDefaultDisplay())
                : manager.createScreenCaptureIntent();
        startActivityForResult(request, CAPTURE);
    }

    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request != CAPTURE) return;
        if (result != RESULT_OK || data == null) { status.setText("已取消录屏授权"); return; }
        try {
            startForegroundService(new Intent(this, CaptureService.class).putExtra("projection", data));
            moveTaskToBack(true);
        } catch (RuntimeException error) { status.setText("无法启动录屏服务"); }
    }

    private Spinner spinner(String[] options, int selection) {
        Spinner spinner = new Spinner(this);
        var adapter = new ArrayAdapter<>(this, android.R.layout.simple_spinner_item, options);
        adapter.setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item);
        spinner.setAdapter(adapter);
        spinner.setSelection(Math.max(0, Math.min(selection, options.length - 1)));
        spinner.setMinimumHeight(dp(44));
        return spinner;
    }

    private int dp(int value) { return HintUi.dp(this, value); }
}
