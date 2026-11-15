import java.io.File;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import se.krka.kahlua.vm.KahluaTable;
import se.krka.kahlua.vm.KahluaTableIterator;
import zombie.ZomboidFileSystem;
import zombie.iso.InstanceTracker;
import zombie.world.moddata.GlobalModData;

// Inside the booted original (ServerHarness): the player's save's global_mod_data.bin and iTrack.bin are copied over
// the server's own, then the ORIGINAL GlobalModData.load and InstanceTracker.load (IsoWorld.init's next steps after
// GameTime.load) read them. The probe prints the global mod data tables in collectTableNames order (the HashMap's), the
// instance groups in their HashMap order, and each group exported as text and as CSV with keys sorted.
// Args: <scratch dir> <save dir>
public class ModDataProbe {
    static String dh(Double d) { return Long.toHexString(Double.doubleToRawLongBits(d)); }

    static String lua(Object v) {
        if (v == null) return "null";
        if (v instanceof String x) return "s:" + x;
        if (v instanceof Double x) return "n:" + dh(x);
        if (v instanceof Boolean x) return "b:" + x;
        if (v instanceof KahluaTable t) {
            List<String> l = new ArrayList<>();
            KahluaTableIterator it = t.iterator();
            while (it.advance()) l.add(lua(it.getKey()) + "=" + lua(it.getValue()));
            Collections.sort(l);
            return "{" + String.join(",", l) + "}";
        }
        return "?" + v;
    }

    public static void main(String[] a) throws Exception {
        File save = new File(a[1]);
        ServerHarness.run(a[0], out -> {
            for (String name : new String[]{"global_mod_data.bin", "iTrack.bin"}) {
                Files.copy(new File(save, name).toPath(), new File(ZomboidFileSystem.instance.getFileNameInCurrentSave(name)).toPath(),
                    StandardCopyOption.REPLACE_EXISTING);
            }
            GlobalModData.instance.load();
            List<String> tags = new ArrayList<>();
            GlobalModData.instance.collectTableNames(tags);
            out.println("GMD " + tags.size());
            for (String t : tags) out.println("GMT " + t + " " + lua(GlobalModData.instance.get(t)));
            InstanceTracker.load();
            Field f = InstanceTracker.class.getDeclaredField("InstanceGroups");
            f.setAccessible(true);
            HashMap<String, ?> groups = (HashMap<String, ?>) f.get(null);
            out.println("IG " + groups.size());
            for (String g : groups.keySet()) {
                out.println("IGN " + g);
                out.print(InstanceTracker.exportGroup(g, InstanceTracker.Format.TEXT, InstanceTracker.Sort.KEY));
                out.print(InstanceTracker.exportGroup(g, InstanceTracker.Format.CSV, InstanceTracker.Sort.KEY));
            }
        });
    }
}
