import fmod.fmod.FMODManager;
import fmod.fmod.FMOD_STUDIO_EVENT_DESCRIPTION;
import fmod.fmod.FMOD_STUDIO_PARAMETER_DESCRIPTION;
import fmod.javafmodJNI;
import java.io.File;
import java.io.PrintStream;
import java.io.RandomAccessFile;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

// The ORIGINAL FMOD sound banks, headless: FMODManager.init() loads media/sound/banks/Desktop/{ZomboidSound.strings,
// ZomboidMusic,ZomboidSound,ZomboidSoundMP}.bank into the Studio system (the natives are the game's own libfmod*.dylib
// of this OS, $PZ_NATIVES) and fills its event and parameter tables. The mode (argument 2) selects the report:
//   banks     the Studio bank list with the event and VCA count the API gives for each bank, then (an independent
//             walk of the files in this probe, no FMOD involved) the RIFF container of every bank file
//   strings   the events of every bank in list order (path and GUID), the VCAs, FMODManager's sorted event path list,
//             the names of its parameter table, and for every candidate path of the file in argument 3 whether
//             FMOD_Studio_System_GetBus / GetVCA know it (pz-dump fmodcandidates writes that file from its own tables)
//   events    FMOD_Studio_System_GetParameterDescriptionList, then every event of FMODManager's path list with what the
//             API says of it (HasSustainPoint, GetLength, and for each parameter GetParameterDescriptionByIndex's
//             name, id and flags; gi is the manager's globalIndex of that name), then the manager's parameter table.
//             The id of a parameter that no event description uses is printed as UNKNOWN: it is not stored in the
//             banks (0 occurrences of its 8 bytes), the hash that makes it is not decoded.
// Native pointers are never printed: the GUID and parameter id the Java wrappers hold are native addresses, read here
// through the foreign memory API (16 and 8 bytes).
public class FmodBanksProbe {
    static final String DIR = "./media/sound/banks/Desktop/";
    static final String[] INIT = {"ZomboidSound.strings", "ZomboidMusic", "ZomboidSound", "ZomboidSoundMP", "Animal", "Broadcast", "Character", "Object",
        "Vehicle", "Weapon", "World", "Zombie"};

    static String hex(long addr, int n) {
        MemorySegment s = MemorySegment.ofAddress(addr).reinterpret(n);
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < n; i++) sb.append(String.format("%02x", s.get(ValueLayout.JAVA_BYTE, i)));
        return sb.toString();
    }

    static long u32(RandomAccessFile f) throws Exception {
        long b0 = f.read(), b1 = f.read(), b2 = f.read(), b3 = f.read();
        return b0 | b1 << 8 | b2 << 16 | b3 << 24;
    }

    static String tag(RandomAccessFile f) throws Exception {
        byte[] t = new byte[4];
        f.readFully(t);
        return new String(t, java.nio.charset.StandardCharsets.ISO_8859_1);
    }

    // the chunks of f between pos and end: [offset, size, tag, listType]
    static List<Object[]> chunks(RandomAccessFile f, long pos, long end) throws Exception {
        List<Object[]> r = new ArrayList<>();
        while (pos + 8 <= end) {
            f.seek(pos);
            String t = tag(f);
            long size = u32(f);
            String lt = t.equals("LIST") ? tag(f) : null;
            r.add(new Object[] {pos, size, t, lt});
            pos += 8 + size + (size & 1);
        }
        return r;
    }

    static void container(PrintStream out, File file) throws Exception {
        String name = file.getName().replaceAll("\\.bank$", "");
        try (RandomAccessFile f = new RandomAccessFile(file, "r")) {
            f.seek(4);
            long riff = u32(f);
            String form = tag(f);
            long end = Math.min(8 + riff, f.length());
            StringBuilder fmt = new StringBuilder();
            List<String> lines = new ArrayList<>();
            for (Object[] c : chunks(f, 12, end)) {
                long off = (Long) c[0], size = (Long) c[1];
                String t = (String) c[2], lt = (String) c[3];
                if (t.equals("FMT ")) {
                    f.seek(off + 8);
                    fmt.append(u32(f)).append(',').append(u32(f));
                } else if (t.equals("SND ")) {
                    lines.add("SND " + name + " offset=" + off + " size=" + size);
                } else if ("PROJ".equals(lt)) {
                    for (Object[] p : chunks(f, off + 12, off + 8 + size)) {
                        long po = (Long) p[0], ps = (Long) p[1];
                        String pt = (String) p[2], plt = (String) p[3];
                        if (pt.equals("BNKI")) {
                            f.seek(po + 8);
                            byte[] g = new byte[16];
                            f.readFully(g);
                            StringBuilder gs = new StringBuilder();
                            for (byte b : g) gs.append(String.format("%02x", b));
                            long z0 = u32(f), z1 = u32(f), ev = u32(f), tl = u32(f);
                            lines.add("BNKI " + name + " guid=" + gs + " zeros=" + Long.toHexString(z0 | z1 << 32) + " events=" + ev + " tail=" + tl);
                        } else if (plt != null) {
                            Map<String, Integer> kids = new LinkedHashMap<>();
                            Long lcnt = null;
                            boolean first = true;
                            for (Object[] k : chunks(f, po + 12, po + 8 + ps)) {
                                String kn = k[3] != null ? "LIST/" + k[3] : (String) k[2];
                                kids.merge(kn, 1, Integer::sum);
                                if (first && k[2].equals("LCNT")) { f.seek((Long) k[0] + 8); lcnt = u32(f); }
                                first = false;
                            }
                            StringBuilder ks = new StringBuilder();
                            for (Map.Entry<String, Integer> e : kids.entrySet()) ks.append(ks.length() > 0 ? "," : "").append(e.getKey()).append(':').append(e.getValue());
                            lines.add("LIST " + name + " " + plt + " size=" + ps + " lcnt=" + (lcnt == null ? "-" : lcnt) + " children=" + (ks.length() == 0 ? "-" : ks));
                        } else {
                            lines.add("TAIL " + name + " " + pt + " size=" + ps);
                        }
                    }
                }
            }
            out.println("FILE " + name + " bytes=" + file.length() + " riff=" + riff + " form=" + form + " fmt=" + fmt);
            for (String l : lines) out.println(l);
        }
    }

    static void banks(PrintStream out) throws Exception {
        FMODManager m = FMODManager.instance;
        int bc = javafmodJNI.FMOD_Studio_System_GetBankCount();
        long[] bl = new long[bc];
        bc = javafmodJNI.FMOD_Studio_System_GetBankList(bl);
        out.println("FMOD init banks=" + bc + " events=" + m.getEventPathList().size());
        List<File> files = new ArrayList<>();
        for (String n : INIT) {
            File f = new File(DIR + n + ".bank");
            if (f.exists()) files.add(f);
        }
        if (files.size() != bc) throw new IllegalStateException("bank files " + files.size() + " != loaded banks " + bc);
        for (int i = 0; i < bc; i++) {
            out.println("BANK " + i + " " + files.get(i).getName().replaceAll("\\.bank$", "") + " events=" + javafmodJNI.FMOD_Studio_Bank_GetEventCount(bl[i])
                + " vcas=" + javafmodJNI.FMOD_Studio_Bank_GetVCACount(bl[i]));
        }
        // radio.bank is not one of the banks init() asks for; loaded here it shows the same counts the header carries
        File radio = new File(DIR + "radio.bank");
        javafmodJNI.FMOD_Studio_LoadBankFile(radio.getCanonicalPath());
        int bc2 = javafmodJNI.FMOD_Studio_System_GetBankCount();
        long[] bl2 = new long[bc2];
        bc2 = javafmodJNI.FMOD_Studio_System_GetBankList(bl2);
        out.println("FMOD radio banks=" + bc2);
        out.println("BANK " + (bc2 - 1) + " radio events=" + javafmodJNI.FMOD_Studio_Bank_GetEventCount(bl2[bc2 - 1]) + " vcas=" + javafmodJNI.FMOD_Studio_Bank_GetVCACount(bl2[bc2 - 1]));
        files.add(radio);
        for (File f : files) container(out, f);
    }

    static void strings(PrintStream out, String candidates) throws Exception {
        FMODManager m = FMODManager.instance;
        int bc = javafmodJNI.FMOD_Studio_System_GetBankCount();
        long[] bl = new long[bc];
        bc = javafmodJNI.FMOD_Studio_System_GetBankList(bl);
        for (int i = 0; i < bc; i++) {
            int n = javafmodJNI.FMOD_Studio_Bank_GetEventCount(bl[i]);
            long[] el = new long[n];
            n = n > 0 ? javafmodJNI.FMOD_Studio_Bank_GetEventList(bl[i], el) : 0;
            for (int j = 0; j < n; j++) {
                out.println("BEV " + i + " " + j + " " + javafmodJNI.FMOD_Studio_EventDescription_GetPath(el[j]) + " " + hex(javafmodJNI.FMOD_Studio_EventDescription_GetID(el[j]), 16));
            }
            int vn = javafmodJNI.FMOD_Studio_Bank_GetVCACount(bl[i]);
            long[] vl = new long[vn];
            vn = vn > 0 ? javafmodJNI.FMOD_Studio_Bank_GetVCAList(bl[i], vl) : 0;
            for (int j = 0; j < vn; j++) out.println("VCA " + i + " " + j + " " + javafmodJNI.FMOD_Studio_VCA_GetPath(vl[j]));
        }
        List<String> paths = m.getEventPathList();
        out.println("EVENTS " + paths.size());
        for (int i = 0; i < paths.size(); i++) out.println("EVT " + i + " " + paths.get(i));
        java.util.TreeSet<String> names = new java.util.TreeSet<>(((Map<String, ?>) g(m, "parameterDescriptionMap")).keySet());
        out.println("PARAMS " + names.size());
        for (String n : names) out.println("PARAM " + n);
        for (String c : java.nio.file.Files.readAllLines(java.nio.file.Path.of(candidates))) {
            out.println("Q " + (javafmodJNI.FMOD_Studio_System_GetBus(c) != 0 ? 1 : 0) + " " + (javafmodJNI.FMOD_Studio_System_GetVCA(c) != 0 ? 1 : 0) + " " + c);
        }
    }

    static void events(PrintStream out) throws Exception {
        FMODManager m = FMODManager.instance;
        int gn = javafmodJNI.FMOD_Studio_System_GetParameterDescriptionCount();
        long[] gl = new long[gn];
        gn = gn > 0 ? javafmodJNI.FMOD_Studio_System_GetParameterDescriptionList(gl) : 0;
        out.println("GLOBALS " + gn);
        for (int i = 0; i < gn; i++) {
            out.println("G " + i + " " + javafmodJNI.FMOD_Studio_ParameterDescription_GetName(gl[i]) + " " + javafmodJNI.FMOD_Studio_ParameterDescription_GetFlags(gl[i]));
            javafmodJNI.FMOD_Studio_ParameterDescription_Free(gl[i]);
        }
        List<String> paths = m.getEventPathList();
        java.util.Set<String> used = new java.util.HashSet<>();
        out.println("EVENTS " + paths.size());
        for (int i = 0; i < paths.size(); i++) {
            FMOD_STUDIO_EVENT_DESCRIPTION d = m.getEventDescription(paths.get(i));
            int pc = javafmodJNI.FMOD_Studio_EventDescription_GetParameterDescriptionCount(d.address);
            if (pc != d.parameters.size()) throw new IllegalStateException("parameter count of " + d.path);
            out.println("EVENT " + i + " " + paths.get(i) + " " + hex(d.id.address(), 16) + " " + (d.hasSustainPoints ? 1 : 0) + " " + d.length + " " + pc);
            for (int k = 0; k < pc; k++) {
                long pd = javafmodJNI.FMOD_Studio_EventDescription_GetParameterDescriptionByIndex(d.address, k);
                String name = javafmodJNI.FMOD_Studio_ParameterDescription_GetName(pd);
                used.add(name);
                out.println("EP " + m.getParameterDescription(name).globalIndex + " " + name + " " + hex(javafmodJNI.FMOD_Studio_ParameterDescription_GetID(pd), 8) + " "
                    + javafmodJNI.FMOD_Studio_ParameterDescription_GetFlags(pd));
                javafmodJNI.FMOD_Studio_ParameterDescription_Free(pd);
            }
        }
        List<FMOD_STUDIO_PARAMETER_DESCRIPTION> pds = new ArrayList<>(((Map<String, FMOD_STUDIO_PARAMETER_DESCRIPTION>) g(m, "parameterDescriptionMap")).values());
        pds.sort(java.util.Comparator.comparingInt(p -> p.globalIndex));
        out.println("PARAMS " + pds.size());
        for (FMOD_STUDIO_PARAMETER_DESCRIPTION p : pds) {
            out.println("PD " + p.globalIndex + " " + p.name + " " + p.flags + " " + (used.contains(p.name) ? hex(p.id.address(), 8) : "UNKNOWN"));
        }
    }

    static Object g(Object o, String name) throws Exception {
        java.lang.reflect.Field f = o.getClass().getDeclaredField(name);
        f.setAccessible(true);
        return f.get(o);
    }

    public static void main(String[] a) throws Exception {
        PrintStream out = new PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
        System.setOut(System.err);
        FMODManager.instance.init();
        String mode = a.length > 1 ? a[1] : "banks";
        switch (mode) {
            case "banks" -> banks(out);
            case "strings" -> strings(out, a[2]);
            case "events" -> events(out);
            default -> throw new IllegalArgumentException("mode " + mode);
        }
        out.flush();
        System.exit(0);
    }
}
