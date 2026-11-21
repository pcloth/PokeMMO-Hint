package local.pokemon.hint;

import android.graphics.Bitmap;
import java.util.Arrays;

final class OcrImage {
    private static final int BORDER = 8;

    private OcrImage() {}

    static Bitmap whiteText(Bitmap image) {
        int width = image.getWidth(), height = image.getHeight();
        int[] source = new int[width * height];
        image.getPixels(source, 0, width, 0, 0, width, height);
        int[] filtered = whiteTextPixels(source, width, height);
        return filtered == null ? null : Bitmap.createBitmap(filtered,
                width + BORDER * 2, height + BORDER * 2, Bitmap.Config.ARGB_8888);
    }

    static int[] whiteTextPixels(int[] source, int width, int height) {
        int stride = width + BORDER * 2;
        int[] mask = new int[stride * (height + BORDER * 2)];
        Arrays.fill(mask, 0xffffffff);
        int radius = Math.max(3, Math.min(10, height / 6));
        int ink = 0;
        for (int y = 0; y < height; y++) {
            int above = Math.max(0, y - radius) * width;
            int below = Math.min(height - 1, y + radius) * width;
            int row = y * width;
            for (int x = 0; x < width; x++) {
                int rgb = source[row + x];
                int r = (rgb >> 16) & 255, g = (rgb >> 8) & 255, b = rgb & 255;
                int low = Math.min(r, Math.min(g, b)), high = Math.max(r, Math.max(g, b));
                if (low < 180 || high - low > 55) continue;

                // A bright stroke stands out locally. Broad, evenly lit fog is
                // usually similar to the pixels a few steps away.
                int background = (luma(source[above + x]) + luma(source[below + x])
                        + luma(source[row + Math.max(0, x - radius)])
                        + luma(source[row + Math.min(width - 1, x + radius)])) / 4;
                if (luma(rgb) - background < 14) continue;
                mask[(y + BORDER) * stride + x + BORDER] = 0xff000000;
                ink++;
            }
        }
        return ink < 8 || ink > source.length * 2 / 5 ? null : mask;
    }

    private static int luma(int rgb) {
        return (77 * ((rgb >> 16) & 255) + 150 * ((rgb >> 8) & 255) + 29 * (rgb & 255)) >> 8;
    }
}
