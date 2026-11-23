package local.pokemon.hint;

import android.content.SharedPreferences;
import java.util.*;

final class History {
    static final String KEY = "history";
    static List<Integer> read(SharedPreferences prefs) {
        List<Integer> result = new ArrayList<>();
        for (String token : prefs.getString(KEY, "").split(",")) {
            try {
                int id = Integer.parseInt(token);
                if (id >= 1 && id <= Dex.MAX_SPECIES && !result.contains(id)) result.add(id);
                if (result.size() == 20) break;
            } catch (NumberFormatException ignored) { }
        }
        return result;
    }

    static void record(SharedPreferences prefs, int id) {
        if (id < 1 || id > Dex.MAX_SPECIES) return;
        var ids = read(prefs);
        if (!ids.isEmpty() && ids.get(0) == id) return;
        ids.remove(Integer.valueOf(id));
        ids.add(0, id);
        if (ids.size() > 20) ids.remove(20);
        StringJoiner saved = new StringJoiner(",");
        for (int value : ids) saved.add(Integer.toString(value));
        prefs.edit().putString(KEY, saved.toString()).apply();
    }
}
