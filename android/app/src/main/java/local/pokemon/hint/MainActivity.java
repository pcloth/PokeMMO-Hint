package local.pokemon.hint;

import android.app.*;
import android.content.*;
import android.graphics.Color;
import android.graphics.Typeface;
import android.media.projection.*;
import android.net.Uri;
import android.os.*;
import android.provider.Settings;
import android.view.*;
import android.widget.*;

public final class MainActivity extends Activity {
    static final String PREFS = "settings";
    static final int[] INTERVALS = {2000, 1000, 500, 250};
    private static final int CAPTURE = 12;
    private TextView status;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        var prefs = getSharedPreferences(PREFS, MODE_PRIVATE);
        ScrollView scroll = new ScrollView(this);
        scroll.setFillViewport(true);
        LinearLayout content = new LinearLayout(this);
        content.setOrientation(LinearLayout.VERTICAL);
        content.setPadding(dp(24), dp(20), dp(24), dp(24));
        content.setBackgroundColor(0xfff2f6fb);
        scroll.addView(content);
        if (Build.VERSION.SDK_INT >= 35) {
            scroll.setOnApplyWindowInsetsListener((view, insets) -> {
                var bars = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
                view.setPadding(bars.left, bars.top, bars.right, bars.bottom);
                return insets;
            });
        }
        setContentView(scroll);
        TextView heading = label("宝可梦属性提示", 30);
        heading.setTypeface(null, Typeface.BOLD);
        content.addView(heading);
        content.addView(label("授权完整屏幕录制后返回游戏。点悬浮栏的 ＋ 框选敌人名字或 No. 编号，再点弱点气泡查看详情。", 16));
        content.addView(label("游戏文字语言", 15));
        Spinner languages = spinner(new String[]{"简体中文", "English", "日本語"}, prefs.getInt("language", 0),
                value -> prefs.edit().putInt("language", value).apply());
        content.addView(languages);
        content.addView(label("识别刷新率", 15));
        content.addView(spinner(new String[]{"0.5 次/秒", "1 次/秒", "2 次/秒", "4 次/秒"}, prefs.getInt("rate", 1),
                value -> prefs.edit().putInt("rate", value).apply()));
        Button start = new Button(this);
        start.setText("开启悬浮识别");
        start.setOnClickListener(view -> requestCapture());
        content.addView(start);
        Button stop = new Button(this);
        stop.setText("结束识别");
        stop.setOnClickListener(view -> { stopService(new Intent(this, CaptureService.class)); status.setText("已结束"); });
        content.addView(stop);
        status = label("", 15);
        content.addView(status);
        content.addView(label("手动查询", 20));
        EditText query = new EditText(this);
        query.setSingleLine();
        query.setHint("名字或全国编号");
        content.addView(query);
        Button find = new Button(this);
        find.setText("查询");
        content.addView(find);
        TextView result = label("", 16);
        result.setTextIsSelectable(true);
        content.addView(result);
        try {
            var assets = getAssets();
            Dex dex = new Dex(assets.open("species.tsv"), assets.open("species_names.tsv"), assets.open("type_chart.txt"));
            find.setOnClickListener(view -> result.setText(formatResult(dex, dex.lookup(query.getText().toString(), true),
                    Dex.Language.values()[languages.getSelectedItemPosition()])));
            Button history = new Button(this);
            history.setText("最近识别");
            history.setOnClickListener(view -> {
                Dex.Language lang = Dex.Language.values()[languages.getSelectedItemPosition()];
                var entries = new java.util.ArrayList<Dex.Species>();
                for (int id : History.read(prefs)) {
                    Dex.Species species = dex.lookup(Integer.toString(id), true);
                    if (species != null) entries.add(species);
                }
                String[] rows = entries.stream().map(entry -> dex.title(entry, lang)).toArray(String[]::new);
                var dialog = new AlertDialog.Builder(this).setTitle("最近识别").setNegativeButton("关闭", null);
                if (rows.length == 0) dialog.setMessage("暂无识别记录");
                else dialog.setItems(rows, (window, which) -> result.setText(formatResult(dex, entries.get(which), lang)));
                dialog.show();
            });
            content.addView(history);
        } catch (Exception error) { result.setText("无法加载本地图鉴"); }
    }

    private String formatResult(Dex dex, Dex.Species entry, Dex.Language language) {
        if (entry == null) return "未匹配到物种";
        StringBuilder display = new StringBuilder(dex.title(entry, language));
        String[] details = dex.details(entry);
        for (int i = 0; i < details.length; i++) {
            display.append("\n").append(CaptureService.GROUPS[i]).append("  ").append(details[i]);
        }
        return display.toString();
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

    private Spinner spinner(String[] options, int selection, java.util.function.IntConsumer changed) {
        Spinner spinner = new Spinner(this);
        var adapter = new ArrayAdapter<>(this, android.R.layout.simple_spinner_dropdown_item, options);
        spinner.setAdapter(adapter);
        spinner.setSelection(Math.max(0, Math.min(selection, options.length - 1)));
        spinner.setOnItemSelectedListener(new AdapterView.OnItemSelectedListener() {
            @Override public void onItemSelected(AdapterView<?> parent, View item, int i, long id) { changed.accept(i); }
            @Override public void onNothingSelected(AdapterView<?> parent) {}
        });
        return spinner;
    }

    private TextView label(String text, int size) {
        TextView view = new TextView(this);
        view.setTextColor(Color.rgb(23, 42, 68));
        view.setTextSize(size);
        view.setText(text);
        view.setPadding(0, dp(8), 0, dp(8));
        return view;
    }

    private int dp(int value) { return Math.round(value * getResources().getDisplayMetrics().density); }
}
