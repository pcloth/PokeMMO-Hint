package local.pokemon.hint;

import android.app.AlertDialog;
import android.content.Context;
import android.graphics.*;
import android.view.*;
import android.widget.*;

final class RegionEditor extends FrameLayout {
    interface Completion { void finish(String name, Rect rect); }
    private final Rect selection = new Rect();
    private final int screenWidth, screenHeight;
    private final Completion done;
    private String name;
    private AlertDialog naming;

    RegionEditor(Context context, int width, int height, Regions.Area existing, Completion done) {
        super(context);
        this.screenWidth = width;
        this.screenHeight = height;
        this.done = done;
        this.name = existing == null ? "" : existing.name();
        if (existing != null) selection.set(existing.rect());
        addView(new SelectionCanvas(context), new LayoutParams(-1, -1));
        LinearLayout panel = new LinearLayout(context);
        panel.setOrientation(LinearLayout.VERTICAL);
        panel.setPadding(dp(8), dp(6), dp(8), dp(6));
        panel.setBackground(Bubble.panel(0xff183253, dp(12)));
        TextView hint = new TextView(context);
        hint.setText("拖框选中名字或编号 · 拖框内移动");
        hint.setTextColor(Color.WHITE);
        hint.setGravity(Gravity.CENTER);
        panel.addView(hint);
        LinearLayout buttons = new LinearLayout(context);
        button(buttons, "命名", this::nameDialog);
        button(buttons, "保存", () -> {
            if (selection.width() < 8 || selection.height() < 8
                    || selection.width() > 2000 || selection.height() > 300) {
                Toast.makeText(context, "请框选一行文字", Toast.LENGTH_SHORT).show();
                return;
            }
            done.finish(name.trim(), new Rect(selection));
        });
        button(buttons, "取消", () -> done.finish("", null));
        panel.addView(buttons);
        LayoutParams layout = new LayoutParams(-2, -2, Gravity.BOTTOM | Gravity.CENTER_HORIZONTAL);
        layout.bottomMargin = dp(28);
        addView(panel, layout);
    }

    private void button(LinearLayout row, String title, Runnable action) {
        Button button = new Button(getContext());
        button.setText(title);
        button.setOnClickListener(v -> action.run());
        row.addView(button);
    }

    private void nameDialog() {
        EditText input = new EditText(getContext());
        input.setSingleLine();
        input.setFilters(new android.text.InputFilter[]{new android.text.InputFilter.LengthFilter(32)});
        input.setHint("例如：群怪左侧");
        input.setText(name);
        naming = new AlertDialog.Builder(getContext()).setTitle("选区名称").setView(input)
                .setPositiveButton("确定", (d, w) -> name = input.getText().toString())
                .setNegativeButton("取消", null).create();
        naming.getWindow().setType(WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY);
        naming.show();
    }

    @Override protected void onDetachedFromWindow() {
        if (naming != null) naming.dismiss();
        super.onDetachedFromWindow();
    }

    private int dp(int size) { return Math.round(size * getResources().getDisplayMetrics().density); }

    private final class SelectionCanvas extends View {
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final Rect original = new Rect();
        private int startX, startY;
        private boolean moving;

        SelectionCanvas(Context context) { super(context); }
        @Override protected void onDraw(Canvas canvas) {
            canvas.drawColor(0x2800142d);
            if (selection.isEmpty()) return;
            int[] origin = new int[2];
            getLocationOnScreen(origin);
            Rect visible = new Rect(selection);
            visible.offset(-origin[0], -origin[1]);
            paint.setColor(0xffffd052);
            paint.setStrokeWidth(dp(2));
            paint.setStyle(Paint.Style.STROKE);
            canvas.drawRect(visible, paint);
        }

        @Override public boolean onTouchEvent(android.view.MotionEvent event) {
            int x = Math.max(0, Math.min(screenWidth, (int) event.getRawX()));
            int y = Math.max(0, Math.min(screenHeight, (int) event.getRawY()));
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN:
                    startX = x; startY = y;
                    original.set(selection);
                    moving = selection.contains(x, y);
                    if (!moving) selection.set(x, y, x, y);
                    break;
                case MotionEvent.ACTION_MOVE:
                case MotionEvent.ACTION_UP:
                    if (moving) {
                        selection.set(original);
                        selection.offsetTo(Math.max(0, Math.min(screenWidth - original.width(), original.left + x - startX)),
                                Math.max(0, Math.min(screenHeight - original.height(), original.top + y - startY)));
                    } else selection.set(Math.min(x, startX), Math.min(y, startY), Math.max(x, startX), Math.max(y, startY));
                    break;
                case MotionEvent.ACTION_CANCEL: selection.set(original); break;
                default: return true;
            }
            invalidate();
            return true;
        }
    }
}
