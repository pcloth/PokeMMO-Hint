package local.pokemon.hint;

import android.content.SharedPreferences;
import android.graphics.Rect;
import org.json.*;
import java.util.*;

final class Regions {
    static final int LIMIT = 16;
    record Area(String name, Rect rect) {}
    static String key(int width, int height, int rotation) { return "regions-" + width + "x" + height + "-" + rotation; }

    static List<Area> load(SharedPreferences prefs, String key, int width, int height) {
        List<Area> result = new ArrayList<>();
        try {
            JSONArray rows = new JSONArray(prefs.getString(key, "[]"));
            for (int i = 0; i < Math.min(rows.length(), LIMIT); i++) {
                JSONArray row = rows.getJSONArray(i);
                Rect rect = new Rect(row.getInt(1), row.getInt(2), row.getInt(3), row.getInt(4));
                if (rect.left >= 0 && rect.top >= 0 && rect.right <= width && rect.bottom <= height
                        && rect.width() >= 8 && rect.height() >= 8 && rect.width() <= 2000 && rect.height() <= 300) result.add(new Area(row.getString(0), rect));
            }
        } catch (JSONException ignored) { /* Retain previously parsed valid entries. */ }
        return result;
    }

    static void save(SharedPreferences prefs, String key, List<Area> areas) {
        JSONArray rows = new JSONArray();
        for (Area area : areas) {
            Rect r = area.rect();
            rows.put(new JSONArray().put(area.name()).put(r.left).put(r.top).put(r.right).put(r.bottom));
        }
        prefs.edit().putString(key, rows.toString()).apply();
    }
}
