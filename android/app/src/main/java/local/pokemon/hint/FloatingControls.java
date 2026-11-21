package local.pokemon.hint;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Rect;
import android.graphics.drawable.Drawable;
import android.os.Handler;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewConfiguration;
import android.widget.LinearLayout;
import android.widget.ImageButton;

final class FloatingControls extends LinearLayout {
    private static final long IDLE_DELAY_MS = 4000;
    private static final int TOOL_COUNT = 5;
    private enum Dock { NONE, LEFT, RIGHT, TOP, BOTTOM }

    private final Rect area;
    private final Handler handler;
    private final Runnable reposition;
    private final Runnable dockWhenIdle = () -> {
        dock = nearestDock(true);
        expanded = false;
        update();
    };
    private final Logo logo;
    private final ImageButton[] buttons;
    private final int diameter;
    private final int buttonWidth;
    private final int touchSlop;
    private WindowPosition position;
    private Dock dock = Dock.NONE;
    private int centerX, centerY, startX, startY;
    private float downX, downY;
    private boolean expanded, dragging, opensLeft;

    record WindowPosition(int x, int y, int width, int height) {}

    FloatingControls(Context context, Rect area, Handler handler, Runnable reposition,
                     Runnable add, Runnable manage, Runnable pause, Runnable home, Runnable stop) {
        super(context);
        this.area = new Rect(area);
        this.handler = handler;
        this.reposition = reposition;
        diameter = Math.min(dp(52), Math.max(2, Math.min(area.width(), area.height())));
        buttonWidth = Math.max(1, Math.min(dp(44), (area.width() - diameter) / TOOL_COUNT));
        touchSlop = ViewConfiguration.get(context).getScaledTouchSlop();
        centerX = area.right - diameter / 2 - dp(12);
        centerY = area.top + diameter / 2 + dp(24);
        setOrientation(HORIZONTAL);
        logo = new Logo(context);
        logo.setOnClickListener(view -> {
            if (expanded) collapse();
            else {
                expanded = true;
                opensLeft = centerX > area.centerX();
                update();
                scheduleDock();
            }
        });
        logo.setOnTouchListener((view, event) -> onLogoTouch(event));
        addView(logo);
        buttons = new ImageButton[]{
                button(R.drawable.tool_add, "添加选区", add),
                button(R.drawable.tool_adjust, "管理选区", manage),
                button(R.drawable.tool_pause, "暂停识别", pause),
                button(R.drawable.tool_home, "打开主界面", home),
                button(R.drawable.tool_cancel, "结束识别", stop)
        };
        update();
    }

    private ImageButton button(int icon, String description, Runnable action) {
        ImageButton button = HintUi.icon(getContext(), icon, description, false, () -> {
            collapse();
            action.run();
        });
        var params = new LayoutParams(buttonWidth, Math.max(1, diameter - dp(8)));
        params.gravity = android.view.Gravity.CENTER_VERTICAL;
        addView(button, params);
        return button;
    }

    void setPaused(boolean paused) {
        buttons[2].setImageResource(paused ? R.drawable.tool_play : R.drawable.tool_pause);
        String title = paused ? "继续识别" : "暂停识别";
        buttons[2].setContentDescription(title);
        buttons[2].setTooltipText(title);
    }

    WindowPosition position() { return position; }

    boolean isDragging() { return dragging; }

    void collapse() {
        if (expanded) {
            expanded = false;
            update();
        }
        scheduleDock();
    }

    private void scheduleDock() {
        handler.removeCallbacks(dockWhenIdle);
        if (expanded || dock == Dock.NONE) handler.postDelayed(dockWhenIdle, IDLE_DELAY_MS);
    }

    @Override protected void onAttachedToWindow() {
        super.onAttachedToWindow();
        scheduleDock();
    }

    @Override protected void onDetachedFromWindow() {
        handler.removeCallbacks(dockWhenIdle);
        super.onDetachedFromWindow();
    }

    @Override public boolean dispatchTouchEvent(MotionEvent event) {
        if (event.getActionMasked() == MotionEvent.ACTION_DOWN) handler.removeCallbacks(dockWhenIdle);
        boolean handled = super.dispatchTouchEvent(event);
        if (event.getActionMasked() == MotionEvent.ACTION_UP
                || event.getActionMasked() == MotionEvent.ACTION_CANCEL) scheduleDock();
        return handled;
    }

    private boolean onLogoTouch(MotionEvent event) {
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                downX = event.getRawX();
                downY = event.getRawY();
                startX = centerX;
                startY = centerY;
                dragging = false;
                logo.setPressed(true);
            }
            case MotionEvent.ACTION_MOVE -> {
                float dx = event.getRawX() - downX;
                float dy = event.getRawY() - downY;
                if (!dragging && Math.hypot(dx, dy) <= touchSlop) return true;
                dragging = true;
                logo.setPressed(false);
                centerX = clamp(startX + Math.round(dx), area.left, area.right);
                centerY = clamp(startY + Math.round(dy), area.top, area.bottom);
                dock = Dock.NONE;
                update();
            }
            case MotionEvent.ACTION_UP -> {
                logo.setPressed(false);
                if (dragging) {
                    dragging = false;
                    dock = nearestDock(false);
                    expanded = false;
                    update();
                } else logo.performClick();
            }
            case MotionEvent.ACTION_CANCEL -> {
                logo.setPressed(false);
                if (dragging) {
                    dragging = false;
                    dock = nearestDock(false);
                    expanded = false;
                    update();
                }
            }
            default -> { }
        }
        return true;
    }

    private Dock nearestDock(boolean anywhere) {
        int[] distances = {area.right - centerX, centerX - area.left,
                centerY - area.top, area.bottom - centerY};
        Dock[] edges = {Dock.RIGHT, Dock.LEFT, Dock.TOP, Dock.BOTTOM};
        int closest = anywhere ? Integer.MAX_VALUE : diameter / 2 + dp(12) + 1;
        Dock edge = Dock.NONE;
        for (int i = 0; i < distances.length; i++) {
            if (distances[i] < closest) {
                closest = distances[i];
                edge = edges[i];
            }
        }
        return edge;
    }

    private void update() {
        Dock visibleDock = expanded || dragging ? Dock.NONE : dock;
        int logoWidth = visibleDock == Dock.LEFT || visibleDock == Dock.RIGHT ? diameter / 2 : diameter;
        int logoHeight = visibleDock == Dock.TOP || visibleDock == Dock.BOTTOM ? diameter / 2 : diameter;
        int rowWidth = expanded ? logoWidth + buttonWidth * buttons.length : logoWidth;
        int rowStart = expanded && opensLeft ? centerX + diameter / 2 - rowWidth : centerX - diameter / 2;
        int x = clamp(rowStart, area.left, area.right - rowWidth);
        int y = clamp(centerY - diameter / 2, area.top, area.bottom - logoHeight);
        if (visibleDock == Dock.LEFT) x = area.left;
        if (visibleDock == Dock.RIGHT) x = area.right - logoWidth;
        if (visibleDock == Dock.TOP) y = area.top;
        if (visibleDock == Dock.BOTTOM) y = area.bottom - logoHeight;
        position = new WindowPosition(x, y, rowWidth, logoHeight);

        var layout = (LayoutParams) logo.getLayoutParams();
        if (layout.width != logoWidth || layout.height != logoHeight) {
            layout.width = logoWidth;
            layout.height = logoHeight;
            logo.setLayoutParams(layout);
        }
        for (ImageButton button : buttons) button.setVisibility(expanded ? VISIBLE : GONE);
        if (expanded && getBackground() == null) setBackground(HintUi.panel(getContext(), HintUi.PAGE, 12));
        else if (!expanded && getBackground() != null) setBackground(null);
        logo.setContentDescription(expanded ? "收起识别工具" : "展开识别工具，可拖动");
        logo.edge = visibleDock;
        logo.invalidate();
        reposition.run();
    }

    @Override protected void onLayout(boolean changed, int left, int top, int right, int bottom) {
        if (!expanded || !opensLeft) {
            super.onLayout(changed, left, top, right, bottom);
            return;
        }
        int x = right - left;
        for (int i = 0; i < getChildCount(); i++) {
            View child = getChildAt(i);
            int width = child.getMeasuredWidth();
            int y = (bottom - top - child.getMeasuredHeight()) / 2;
            child.layout(x - width, y, x, y + child.getMeasuredHeight());
            x -= width;
        }
    }

    private static int clamp(int value, int min, int max) {
        return Math.max(min, Math.min(max, value));
    }

    private int dp(int value) {
        return Math.round(value * getResources().getDisplayMetrics().density);
    }

    private final class Logo extends View {
        private final Drawable icon;
        private Dock edge = Dock.NONE;

        Logo(Context context) {
            super(context);
            icon = context.getDrawable(R.drawable.app_icon);
            setFocusable(true);
        }

        @Override protected void onDraw(Canvas canvas) {
            super.onDraw(canvas);
            if (icon == null) return;

            float w = getWidth(), h = getHeight();
            float x = edge == Dock.LEFT ? 0 : edge == Dock.RIGHT ? w : w / 2;
            float y = edge == Dock.TOP ? 0 : edge == Dock.BOTTOM ? h : h / 2;
            int size = expanded ? Math.round(diameter * 0.76f) : diameter;
            int left = Math.round(x - size / 2f);
            int top = Math.round(y - size / 2f);

            // Keep the same icon size when docked. The smaller window crops the
            // half that would extend past the screen edge.
            int saved = canvas.save();
            canvas.clipRect(0, 0, w, h);
            icon.setAlpha(expanded ? 255 : 204);
            icon.setBounds(left, top, left + size, top + size);
            icon.draw(canvas);
            canvas.restoreToCount(saved);
        }
    }
}
