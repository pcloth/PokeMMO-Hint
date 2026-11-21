package local.pokemon.hint;

import android.content.Context;
import android.content.res.ColorStateList;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.RippleDrawable;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.ImageButton;
import android.widget.LinearLayout;
import android.widget.TextView;

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

    static View card(Context c, Dex dex, Dex.Species entry, Dex.Language language, Runnable close) {
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
        View rule = new View(c);
        rule.setBackgroundColor(BORDER);
        LinearLayout.LayoutParams separator = new LinearLayout.LayoutParams(-1, dp(c, 1));
        separator.topMargin = dp(c, 12);
        card.addView(rule, separator);

        String[] groups = dex.details(entry);
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
