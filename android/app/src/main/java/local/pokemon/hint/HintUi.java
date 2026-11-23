package local.pokemon.hint;

import android.app.AlertDialog;
import android.content.Context;
import android.content.res.ColorStateList;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.RippleDrawable;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.AdapterView;
import android.widget.ArrayAdapter;
import android.widget.Button;
import android.widget.CheckBox;
import android.widget.ImageButton;
import android.widget.LinearLayout;
import android.widget.TextView;
import android.widget.Spinner;
import android.widget.ScrollView;
import java.util.ArrayList;
import java.util.StringJoiner;
import java.util.function.Consumer;

final class HintUi {
    static final int PAGE = 0xfff3f6fb, INK = 0xff1e293b, MUTED = 0xff64748b, BORDER = 0xffe0e7f0;
    static final int WHITE = 0xffffffff;
    private static final int[] COLORS = {0xffb91c1c, MUTED, 0xff047857, 0xff6d28d9};
    private static final int[] FILLS = {0xffffefef, 0xfff1f5f9, 0xffe8f8f2, 0xfff2eeff};
    private static final String[] GROUPS = {"弱点", "普通伤害", "抗性", "免疫"};

    private HintUi() {}

    static int dp(Context c, int size) {
        return Math.round(size * c.getResources().getDisplayMetrics().density);
    }

    static GradientDrawable panel(Context c, int color, int radius) {
        GradientDrawable background = Bubble.panel(color, dp(c, radius));
        background.setStroke(dp(c, 1), BORDER);
        return background;
    }

    static TextView text(Context c, String value, int size, int color, boolean bold) {
        TextView text = new TextView(c);
        text.setText(value);
        text.setTextSize(size);
        text.setTextColor(color);
        if (bold) text.setTypeface(null, Typeface.BOLD);
        return text;
    }

    static LinearLayout column(Context c) {
        LinearLayout column = new LinearLayout(c);
        column.setOrientation(LinearLayout.VERTICAL);
        return column;
    }

    static ImageButton icon(Context c, int icon, String title, boolean primary, Runnable action) {
        ImageButton button = new ImageButton(c);
        button.setImageResource(icon);
        button.setContentDescription(title);
        button.setTooltipText(title);
        button.setScaleType(android.widget.ImageView.ScaleType.FIT_CENTER);
        int padding = dp(c, 9);
        button.setPadding(padding, padding, padding, padding);
        button.setBackground(new RippleDrawable(ColorStateList.valueOf(0x25175bc4),
                panel(c, primary ? 0xffe8f1ff : WHITE, 9), null));
        button.setOnClickListener(view -> action.run());
        return button;
    }

    static View resultCard(Context c, Dex.Card result, Dex.Language language, Runnable close,
                           Consumer<Dex.Typing> apply, Consumer<AlertDialog> showDialog) {
        LinearLayout container = column(c);
        LinearLayout tabs = new LinearLayout(c);
        tabs.setPadding(dp(c, 10), dp(c, 8), dp(c, 10), 0);
        for (Dex.Tab tab : Dex.Tab.values()) {
            boolean selected = tab == result.tab();
            boolean enabled = result.supports(tab);
            Button button = resultButton(c, tab.label, enabled, selected);
            button.setOnClickListener(view -> {
                if (!selected) apply.accept(result.typing().withTab(tab));
            });
            var layout = new LinearLayout.LayoutParams(0, -1, 1);
            if (tab != Dex.Tab.LEGACY) layout.leftMargin = dp(c, 4);
            tabs.addView(button, layout);
        }
        container.addView(tabs);

        String option = switch (result.tab()) {
            case LEGACY -> null;
            case MODERN -> result.rules().forms(result.base()).isEmpty() ? null : "选择形态";
            case TERA -> "选择太晶属性";
            case DYNAMAX -> result.rules().canGigantamax(result.base(), result.typing().form())
                    || result.rules().forms(result.base()).stream().anyMatch(form -> result.rules().dynamaxForm(result.base(), form))
                    ? "选择极巨形态" : null;
        };
        if (option != null) {
            Button button = resultButton(c, option, true, false);
            button.setPadding(dp(c, 10), 0, dp(c, 10), 0);
            button.setOnClickListener(view -> showDialog.accept(typingDialog(c, result, language, apply)));
            var layout = new LinearLayout.LayoutParams(-2, -2);
            layout.setMargins(dp(c, 10), dp(c, 6), dp(c, 10), 0);
            container.addView(button, layout);
        }
        container.addView(card(c, result, language, close));
        return container;
    }

    private static Button resultButton(Context c, String label, boolean enabled, boolean selected) {
        Button button = new Button(c, null, 0);
        button.setAllCaps(false);
        button.setText(label);
        button.setTextSize(13);
        button.setTextColor(enabled ? INK : MUTED);
        button.setMinWidth(0);
        button.setMinimumWidth(0);
        button.setPadding(dp(c, 2), 0, dp(c, 2), 0);
        button.setMinimumHeight(dp(c, 44));
        button.setBackground(new RippleDrawable(ColorStateList.valueOf(0x25175bc4),
                panel(c, selected ? 0xffe8f1ff : enabled ? WHITE : PAGE, 8), null));
        button.setSelected(selected);
        button.setEnabled(enabled);
        return button;
    }

    private static View card(Context c, Dex.Card result, Dex.Language language, Runnable close) {
        Dex.Species entry = result.entry();
        LinearLayout card = column(c);
        card.setPadding(dp(c, 16), dp(c, 12), dp(c, 16), dp(c, 14));
        LinearLayout header = new LinearLayout(c);
        header.setGravity(Gravity.CENTER_VERTICAL);
        String title = String.format(java.util.Locale.ROOT, "#%03d  %s", entry.id(), entry.name(language));
        header.addView(text(c, title, 21, INK, true), new LinearLayout.LayoutParams(0, -2, 1));
        if (close != null) {
            header.addView(icon(c, R.drawable.tool_cancel, "关闭详情", false, close),
                    new LinearLayout.LayoutParams(dp(c, 44), dp(c, 44)));
        }
        card.addView(header);
        Chips types = new Chips(c);
        for (int type : entry.types()) types.addView(chip(c, Dex.typeName(type), 0xffedf2fa, 0xff295995));
        card.addView(types, new LinearLayout.LayoutParams(-1, -2));
        String note = switch (result.tab()) {
            case TERA -> switch (result.rules().activeTera(result.base(), result.typing())) {
                case -1 -> "请选择太晶属性，当前显示太晶化前的相克。";
                case Dex.Typing.STELLAR -> "星晶保留太晶化前的防守属性。";
                default -> "";
            };
            case DYNAMAX -> "极巨化和超极巨化保留原形态的防守属性。";
            default -> "";
        };
        if (!note.isEmpty()) {
            TextView caption = text(c, note, 12, MUTED, false);
            caption.setPadding(0, dp(c, 6), 0, 0);
            card.addView(caption);
        }
        View rule = new View(c);
        rule.setBackgroundColor(BORDER);
        LinearLayout.LayoutParams separator = new LinearLayout.LayoutParams(-1, dp(c, 1));
        separator.topMargin = dp(c, 12);
        card.addView(rule, separator);

        String[] groups = result.rules().details(entry);
        for (int i = 0; i < GROUPS.length; i++) {
            LinearLayout row = new LinearLayout(c);
            row.setPadding(0, dp(c, 10), 0, 0);
            TextView name = text(c, GROUPS[i], 13, COLORS[i], true);
            name.setPadding(0, dp(c, 5), dp(c, 6), 0);
            row.addView(name, new LinearLayout.LayoutParams(dp(c, 68), -2));
            Chips values = new Chips(c);
            for (String value : groups[i].split("、")) values.addView(chip(c, value, FILLS[i], COLORS[i]));
            row.addView(values, new LinearLayout.LayoutParams(0, -2, 1));
            card.addView(row, new LinearLayout.LayoutParams(-1, -2));
        }
        return card;
    }

    private static AlertDialog typingDialog(Context c, Dex.Card result, Dex.Language language,
                                            Consumer<Dex.Typing> apply) {
        LinearLayout body = column(c);
        body.setPadding(dp(c, 22), dp(c, 8), dp(c, 22), dp(c, 8));
        Dex dex = result.rules();
        Dex.Species entry = result.base();
        Dex.Typing current = result.typing();
        Dex.Tab tab = result.tab();
        Runnable commit;
        String title;
        if (tab == Dex.Tab.TERA) {
            title = "选择太晶属性";
            var ids = new ArrayList<Integer>();
            var labels = new ArrayList<String>();
            ids.add(-1);
            labels.add("未太晶化");
            int fixed = dex.fixedTera(entry, current.form());
            for (int type = 0; type <= Dex.Typing.STELLAR; type++) {
                if (fixed >= 0 && type != fixed) continue;
                ids.add(type);
                labels.add(type == Dex.Typing.STELLAR ? "星晶" : Dex.typeName(type));
            }
            Spinner choices = choices(c, labels.toArray(new String[0]),
                    ids.indexOf(dex.activeTera(entry, current)));
            body.addView(choices);
            commit = () -> apply.accept(new Dex.Typing(tab, current.form(),
                    ids.get(choices.getSelectedItemPosition()), current.gigantamax()));
        } else {
            boolean dynamax = tab == Dex.Tab.DYNAMAX;
            title = dynamax ? "选择极巨形态" : "选择形态";
            var ids = new ArrayList<Integer>();
            var labels = new ArrayList<String>();
            ids.add(-1);
            labels.add("默认形态");
            var forms = dex.forms(entry);
            for (int i = 0; i < forms.size(); i++) {
                Dex.Form form = forms.get(i);
                if (dynamax && !dex.dynamaxForm(entry, form)) continue;
                var types = new StringJoiner(" / ");
                for (int type : form.types()) types.add(Dex.typeName(type));
                ids.add(i);
                labels.add(form.name(language) + " · " + types);
            }
            Spinner choices = choices(c, labels.toArray(new String[0]), ids.indexOf(current.form()));
            if (ids.size() > 1) body.addView(choices);
            CheckBox gigantamax = new CheckBox(c);
            if (dynamax && ids.stream().anyMatch(index -> dex.canGigantamax(entry, index))) {
                gigantamax.setText("超极巨化");
                gigantamax.setChecked(current.gigantamax());
                body.addView(gigantamax);
                choices.setOnItemSelectedListener(new AdapterView.OnItemSelectedListener() {
                    @Override public void onItemSelected(AdapterView<?> parent, View view, int position, long itemId) {
                        gigantamax.setEnabled(dex.canGigantamax(entry, ids.get(position)));
                    }
                    @Override public void onNothingSelected(AdapterView<?> parent) {}
                });
            }
            commit = () -> {
                int form = ids.get(Math.max(0, choices.getSelectedItemPosition()));
                boolean gmax = dynamax ? gigantamax.isChecked() && dex.canGigantamax(entry, form) : current.gigantamax();
                apply.accept(new Dex.Typing(tab, form, current.tera(), gmax));
            };
        }
        ScrollView scroll = new ScrollView(c);
        scroll.addView(body);
        return new AlertDialog.Builder(c).setTitle(title).setView(scroll)
                .setNegativeButton("取消", null).setPositiveButton("应用", (dialog, button) -> commit.run())
                .create();
    }

    private static Spinner choices(Context c, String[] labels, int selected) {
        Spinner spinner = new Spinner(c);
        var adapter = new ArrayAdapter<>(c, android.R.layout.simple_spinner_item, labels);
        adapter.setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item);
        spinner.setAdapter(adapter);
        spinner.setSelection(Math.max(0, selected));
        spinner.setMinimumHeight(dp(c, 44));
        return spinner;
    }

    private static TextView chip(Context c, String value, int fill, int ink) {
        TextView chip = text(c, value, 13, ink, false);
        chip.setPadding(dp(c, 8), dp(c, 4), dp(c, 8), dp(c, 4));
        chip.setBackground(Bubble.panel(fill, dp(c, 6)));
        return chip;
    }

    /** A compact row of chips that wraps at whole chip boundaries. */
    private static final class Chips extends ViewGroup {
        private final int gap;

        Chips(Context c) {
            super(c);
            gap = dp(c, 5);
        }

        @Override protected void onMeasure(int width, int height) {
            int available = MeasureSpec.getSize(width);
            for (int i = 0; i < getChildCount(); i++) {
                getChildAt(i).measure(MeasureSpec.makeMeasureSpec(available, MeasureSpec.AT_MOST),
                        MeasureSpec.makeMeasureSpec(0, MeasureSpec.UNSPECIFIED));
            }
            setMeasuredDimension(available, arrange(available, false));
        }

        @Override protected void onLayout(boolean changed, int left, int top, int right, int bottom) {
            arrange(right - left, true);
        }

        private int arrange(int width, boolean layout) {
            int x = 0, y = 0, lineHeight = 0;
            for (int i = 0; i < getChildCount(); i++) {
                View chip = getChildAt(i);
                int w = chip.getMeasuredWidth(), h = chip.getMeasuredHeight();
                if (x != 0 && x + w > width) {
                    x = 0;
                    y += lineHeight + gap;
                    lineHeight = 0;
                }
                if (layout) chip.layout(x, y, x + w, y + h);
                x += w + gap;
                lineHeight = Math.max(lineHeight, h);
            }
            return y + lineHeight;
        }

        @Override protected LayoutParams generateDefaultLayoutParams() {
            return new LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT);
        }
    }
}
