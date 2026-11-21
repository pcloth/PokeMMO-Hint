package local.pokemon.hint;

import android.content.Context;
import android.graphics.*;
import android.graphics.drawable.GradientDrawable;
import android.widget.FrameLayout;

final class Bubble extends FrameLayout {
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Path silhouette = new Path();
    private final Rect target, window;
    private final int fill, edge, depth, side;
    private static final int TOP = 0, BOTTOM = 1, LEFT = 2, RIGHT = 3;

    Bubble(Context context, Rect window, Rect target, int fill, int edge, int depth) {
        super(context);
        this.window = new Rect(window);
        this.target = new Rect(target);
        this.fill = fill;
        this.edge = edge;
        this.depth = depth;
        side = window.bottom <= target.top ? BOTTOM : window.top >= target.bottom ? TOP
                : window.right <= target.left ? RIGHT : LEFT;
        setWillNotDraw(false);
        setPadding(side == LEFT ? depth : 0, side == TOP ? depth : 0,
                side == RIGHT ? depth : 0, side == BOTTOM ? depth : 0);
    }

    static GradientDrawable panel(int fill, int radius) {
        GradientDrawable drawable = new GradientDrawable();
        drawable.setColor(fill);
        drawable.setCornerRadius(radius);
        return drawable;
    }

    @Override protected void onSizeChanged(int width, int height, int oldWidth, int oldHeight) {
        float radius = depth * 1.15f;
        RectF body = new RectF(1 + getPaddingLeft(), 1 + getPaddingTop(),
                width - getPaddingRight() - 1, height - getPaddingBottom() - 1);
        silhouette.reset();
        silhouette.addRoundRect(body, radius, radius, Path.Direction.CW);
        Path triangle = new Path();
        if (side == TOP || side == BOTTOM) {
            float half = Math.min(depth, body.width() / 5);
            float x = clamp(target.centerX() - window.left, body.left + radius + half, body.right - radius - half);
            float base = side == TOP ? body.top + 2 : body.bottom - 2;
            triangle.moveTo(x - half, base);
            triangle.lineTo(x, side == TOP ? 1 : height - 1);
            triangle.lineTo(x + half, base);
        } else {
            float half = Math.min(depth, body.height() / 5);
            float y = clamp(target.centerY() - window.top, body.top + radius + half, body.bottom - radius - half);
            float base = side == LEFT ? body.left + 2 : body.right - 2;
            triangle.moveTo(base, y - half);
            triangle.lineTo(side == LEFT ? 1 : width - 1, y);
            triangle.lineTo(base, y + half);
        }
        triangle.close();
        silhouette.op(triangle, Path.Op.UNION);
    }

    private static float clamp(float value, float min, float max) {
        return min > max ? (min + max) / 2 : Math.max(min, Math.min(max, value));
    }

    @Override protected void onDraw(Canvas canvas) {
        paint.setStyle(Paint.Style.FILL);
        paint.setColor(fill);
        canvas.drawPath(silhouette, paint);
        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth(Math.max(1, depth / 6f));
        paint.setColor(edge);
        canvas.drawPath(silhouette, paint);
    }
}
