// Name an MSVC x86 program's functions from what the exe says about itself: RTTI and assert strings.
//
// Run by:  ./kit ghidra analyse -o <name>   (as the import's post-script)
//          ./kit ghidra name -o <name>      (again, any time; it only renames default-named functions)
//
// No symbols ship with a retail Windows exe, but MSVC leaves two things behind that name most of the code
// a port cares about:
//
// * RTTI. Every polymorphic class has a type descriptor (`.?AVMotocrossRider@@`), a hierarchy descriptor
//   (its base classes, in order) and, one dword before each of its vtables, a pointer to a "complete object
//   locator" naming the class. So every vtable is found and named here from the raw image — not from the
//   analyser's labels, which vary between Ghidra versions — and each slot's function is moved into its
//   class's namespace as `vfN`. A function several vtables share (an inherited, not overridden, method)
//   goes to the most basic class that has it: the one with the fewest bases.
// * Assert strings. `MotocrossRider.cpp` in a string means the function that reads it was compiled from
//   that file. A function still default-named after the vtable pass becomes `<File>_cpp_<address>`, and
//   every such function gets a `source:` plate comment either way.
//
// Writes <decomp dir>/<program>.vtables.tsv (every slot), .sources.tsv (functions per file) and
// <program>.classes.tsv beside the dumps — class, vtable, its offset in the object,
// slot count, bases (self first, depth first, each as Name/<how many bases it has>) — the input of research/mxvatv-pc.md's class map. The decomp
// directory is taken from the first argument, else the project's sibling `decomp`. The second argument is
// the oracle file: its rows for this program (`name pc 0xaddr`) name their functions, over any name above.
//@category PS2
import ghidra.app.script.GhidraScript;
import ghidra.app.util.NamespaceUtils;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.Namespace;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.SourceType;

import java.io.File;
import java.io.PrintWriter;
import java.util.*;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class NameOther extends GhidraScript {
    Memory mem;

    static class Vt {
        long addr, col;
        int offset, slots;
        String cls;
        List<String> bases = new ArrayList<>();
        List<Long> fns = new ArrayList<>();
    }

    @Override
    public void run() throws Exception {
        mem = currentProgram.getMemory();
        String[] a = getScriptArgs();
        File out = a.length > 0 ? new File(a[0])
                : new File(new File(getState().getProject().getProjectLocator().getLocation()).getParentFile(),
                           "decomp");
        String program = getState().getProject().getProjectLocator().getName();   // [other.<name>]
        out.mkdirs();

        // 1. Complete object locators: sig 0, offset, cdOffset, type descriptor, hierarchy descriptor.
        Map<Long, Vt> cols = new HashMap<>();
        for (MemoryBlock b : mem.getBlocks()) {
            if (!b.isInitialized() || b.isExecute()) continue;
            long s = b.getStart().getOffset(), e = b.getEnd().getOffset();
            for (long p = (s + 3) & ~3L; p + 20 <= e; p += 4) {
                if (monitor.isCancelled()) return;
                if (u32(p) != 0) continue;
                long td = u32(p + 12), chd = u32(p + 16);
                String name = typeName(td);
                if (name == null || !mapped(chd) || u32(chd) != 0) continue;
                Vt v = new Vt();
                v.col = p;
                v.offset = (int) u32(p + 4);
                v.cls = name;
                long n = u32(chd + 8), bca = u32(chd + 12);
                for (int i = 0; i < n && i < 64 && mapped(bca); i++) {
                    long bcd = u32(bca + 4L * i);
                    String bn = typeName(u32(bcd));
                    // numContainedBases rides along, so the flattened array can be read back as a tree:
                    // a base's own bases are the next N entries, then its next sibling.
                    v.bases.add((bn == null ? "?" : bn) + "/" + u32(bcd + 4));
                }
                cols.put(p, v);
            }
        }

        // 2. Vtables: a dword pointing at a locator, followed by pointers into code.
        List<Vt> vts = new ArrayList<>();
        for (MemoryBlock b : mem.getBlocks()) {
            if (!b.isInitialized() || b.isExecute()) continue;
            long s = b.getStart().getOffset(), e = b.getEnd().getOffset();
            for (long p = (s + 3) & ~3L; p + 8 <= e; p += 4) {
                Vt c = cols.get(u32(p));
                if (c == null || !code(u32(p + 4))) continue;
                Vt v = new Vt();
                v.addr = p + 4;
                v.col = c.col;
                v.offset = c.offset;
                v.cls = c.cls;
                v.bases = c.bases;
                for (long q = v.addr; q + 4 <= e; q += 4) {
                    if (q > v.addr && hasLabelOrRef(q)) break;     // the next vtable's or table's start
                    long f = u32(q);
                    if (!code(f)) break;
                    v.fns.add(f);
                }
                v.slots = v.fns.size();
                vts.add(v);
            }
        }

        // 3. Owner of each function: the most basic class whose vtable holds it (fewest bases), then name.
        Map<Long, Vt> owner = new HashMap<>();
        Map<Long, Integer> slot = new HashMap<>();
        for (Vt v : vts) {
            for (int i = 0; i < v.fns.size(); i++) {
                long f = v.fns.get(i);
                Vt o = owner.get(f);
                if (o == null || v.bases.size() < o.bases.size()
                        || (v.bases.size() == o.bases.size() && v.cls.compareTo(o.cls) < 0)) {
                    owner.put(f, v);
                    slot.put(f, i);
                }
            }
        }
        int named = 0, made = 0;
        for (Vt v : vts) {
            Namespace ns = NamespaceUtils.createNamespaceHierarchy(v.cls, null, currentProgram,
                                                                   SourceType.ANALYSIS);
            Address va = toAddr(v.addr);
            if (currentProgram.getSymbolTable().getSymbols(va).length == 0
                    || currentProgram.getSymbolTable().getPrimarySymbol(va).getSource() == SourceType.DEFAULT) {
                createLabel(va, v.offset == 0 ? "vftable" : "vftable_at_" + v.offset, ns, true,
                            SourceType.ANALYSIS);
            }
        }
        for (Map.Entry<Long, Vt> e : owner.entrySet()) {
            Address fa = toAddr(e.getKey());
            Function f = getFunctionAt(fa);
            if (f == null) {
                disassemble(fa);
                f = createFunction(fa, null);
                if (f == null) continue;
                made++;
            }
            if (f.getSymbol().getSource() != SourceType.DEFAULT) continue;
            Vt v = e.getValue();
            Namespace ns = NamespaceUtils.createNamespaceHierarchy(v.cls, null, currentProgram,
                                                                   SourceType.ANALYSIS);
            String n = "vf" + slot.get(e.getKey()) + (v.offset == 0 ? "" : "_at_" + v.offset);
            try {
                f.getSymbol().setNameAndNamespace(n, ns, SourceType.ANALYSIS);
                named++;
            } catch (Exception ex) {
                println("NameOther: " + v.cls + "::" + n + ": " + ex.getMessage());
            }
        }

        // Functions made here were never analysed: without this their `push offset ".\\File.cpp"` has no
        // reference to the string, and the pass below would miss every assert in them.
        if (made > 0) analyzeChanges(currentProgram);

        // 4. Assert source files. Read off the raw image, not the defined strings: the analyser leaves
        // many unlabelled. The string a call pushes starts somewhere in the printable run before the name
        // (`.\\MotocrossRider.cpp`), so every start in the run is tried.
        Pattern cpp = Pattern.compile("([A-Za-z0-9_]+)\\.(cpp|c|h|hpp|inl)\u0000");
        int tagged = 0, byFile = 0;
        Map<String, Integer> files = new TreeMap<>();
        Set<String> seen = new HashSet<>();
        for (MemoryBlock b : mem.getBlocks()) {
            if (!b.isInitialized() || b.isExecute() || b.getSize() > (64 << 20)) continue;
            byte[] buf = new byte[(int) b.getSize()];
            b.getBytes(b.getStart(), buf);
            String text = new String(buf, java.nio.charset.StandardCharsets.ISO_8859_1);
            Matcher m = cpp.matcher(text);
            while (m.find()) {
                if (monitor.isCancelled()) break;
                String file = m.group(1) + "." + m.group(2);
                int from = m.start();
                while (from > 0 && m.start() - from < 256 && text.charAt(from - 1) >= 0x20
                        && text.charAt(from - 1) < 0x7f) from--;
                for (int at = from; at <= m.start(); at++) {
                    for (Reference r : getReferencesTo(b.getStart().add(at))) {
                        Function f = getFunctionContaining(r.getFromAddress());
                        if (f == null) continue;
                        if (seen.add(file + "@" + f.getEntryPoint())) files.merge(file, 1, Integer::sum);
                        String plate = f.getComment();
                        if (plate == null || !plate.contains("source: " + file)) {
                            f.setComment((plate == null ? "" : plate + "\n") + "source: " + file);
                            tagged++;
                        }
                        if (f.getSymbol().getSource() == SourceType.DEFAULT) {
                            f.setName(m.group(1) + "_" + m.group(2) + "_" + String.format("%08x",
                                      f.getEntryPoint().getOffset()), SourceType.ANALYSIS);
                            byFile++;
                        }
                    }
                }
            }
        }

        // 5. The oracle's rows for this program (`pc_engine_update pc 0x… note`), matched by hand to
        // the PS2 functions they port: the name goes on as the user's, inside whatever class namespace
        // the vtable pass gave the function, so `./kit fn pc_engine_update -o pc` finds it.
        int fromOracle = 0;
        File oracle = a.length > 1 ? new File(a[1]) : null;
        if (oracle != null && oracle.exists()) {
            for (String line : java.nio.file.Files.readAllLines(oracle.toPath())) {
                String[] t = line.split("#", 2)[0].trim().split("\\s+");
                if (t.length < 3 || !t[1].equals(program)) continue;
                Function f = getFunctionAt(toAddr(Long.parseLong(t[2].replaceFirst("^0[xX]", ""), 16)));
                if (f == null) {
                    println("NameOther: " + t[0] + ": no function at " + t[2]);
                    continue;
                }
                if (!f.getName().equals(t[0])) {
                    f.setName(t[0], SourceType.USER_DEFINED);
                    fromOracle++;
                }
            }
        }

        File tsv = new File(out, program + ".classes.tsv");
        try (PrintWriter w = new PrintWriter(tsv, "UTF-8")) {
            w.println("# class\tvftable\toffset\tslots\tbases (self first, depth first, Name/<its base count>)");
            vts.sort(Comparator.comparing((Vt v) -> v.cls).thenComparing(v -> v.offset));
            for (Vt v : vts) {
                w.println(String.format("%s\t%08x\t%d\t%d\t%s", v.cls, v.addr, v.offset, v.slots,
                                        String.join(",", v.bases)));
            }
        }
        // Every slot of every vtable, so a vtable can be laid beside another build's slot by slot.
        try (PrintWriter w = new PrintWriter(new File(out, program + ".vtables.tsv"), "UTF-8")) {
            for (Vt v : vts) {
                for (int i = 0; i < v.fns.size(); i++) {
                    Function f = getFunctionAt(toAddr(v.fns.get(i)));
                    w.println(String.format("%08x\t%s\t%d\t%d\t%08x\t%s", v.addr, v.cls, v.offset, i,
                            v.fns.get(i), f == null ? "-" : f.getName(true).replace(' ', '_')));
                }
            }
        }
        File src = new File(out, program + ".sources.tsv");
        try (PrintWriter w = new PrintWriter(src, "UTF-8")) {
            for (Map.Entry<String, Integer> e : files.entrySet()) w.println(e.getKey() + "\t" + e.getValue());
        }
        println(String.format("NameOther: %d locators, %d vtables, %d slot functions named (%d created), "
                + "%d newly tagged with a source file, %d files (%d named by file), %d named from the oracle -> %s",
                cols.size(), vts.size(), named, made, tagged, files.size(), byFile, fromOracle, tsv));
    }

    /** `.?AVMotocrossRider@@` at type descriptor + 8 → `MotocrossRider`; nested `A@B@@` → `B::A`. */
    String typeName(long td) {
        if (!mapped(td) || !mapped(td + 8)) return null;
        StringBuilder sb = new StringBuilder();
        try {
            for (long p = td + 8; sb.length() < 512; p++) {
                byte b = mem.getByte(toAddr(p));
                if (b == 0) break;
                sb.append((char) (b & 0xff));
            }
        } catch (Exception e) {
            return null;
        }
        String s = sb.toString();
        if (!(s.startsWith(".?AV") || s.startsWith(".?AU")) || !s.endsWith("@@")) return null;
        s = s.substring(4, s.length() - 2);
        if (s.contains("?") || s.contains("$")) return s.replaceAll("[^A-Za-z0-9_@]", "_").replace('@', '_');
        String[] parts = s.split("@");
        Collections.reverse(Arrays.asList(parts));
        return String.join("::", parts);
    }

    boolean hasLabelOrRef(long p) {
        Address a = toAddr(p);
        if (currentProgram.getReferenceManager().hasReferencesTo(a)) return true;
        return false;
    }

    long u32(long p) {
        try {
            return mem.getInt(toAddr(p)) & 0xffffffffL;
        } catch (Exception e) {
            return -1;
        }
    }

    boolean mapped(long p) {
        if (p <= 0 || p > 0xffffffffL) return false;
        MemoryBlock b = mem.getBlock(toAddr(p));
        return b != null && b.isInitialized();
    }

    boolean code(long p) {
        if (p <= 0 || p > 0xffffffffL) return false;
        MemoryBlock b = mem.getBlock(toAddr(p));
        return b != null && b.isExecute() && b.isInitialized();
    }
}
