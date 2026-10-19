// Applies Cpp2IL method names (method_addresses.tsv) to the open program.
// Usage (headless): -postScript ApplyIl2cppNames.java /path/to/method_addresses.tsv
//@category IL2CPP
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.SourceType;
import java.io.BufferedReader;
import java.io.FileReader;
import java.util.HashSet;
import java.util.Set;

public class ApplyIl2cppNames extends GhidraScript {
    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        long base = currentProgram.getImageBase().getOffset();
        println("image base 0x" + Long.toHexString(base));
        int total = 0, named = 0, created = 0, noCode = 0, failed = 0;
        Set<String> used = new HashSet<>();
        try (BufferedReader r = new BufferedReader(new FileReader(args[0]))) {
            r.readLine(); // header
            String line;
            while ((line = r.readLine()) != null) {
                String[] f = line.split("\t");
                if (f.length < 5) continue;
                total++;
                long va = Long.parseUnsignedLong(f[0].substring(2), 16);
                Address a = toAddr(base + va);
                String name = clean(f[3]) + "__" + clean(f[4]);
                if (!used.add(name)) name = name + "_" + Long.toHexString(va);
                Function fn = getFunctionAt(a);
                if (fn == null) {
                    if (getInstructionAt(a) == null) disassemble(a);
                    if (getInstructionAt(a) == null) { noCode++; continue; }
                    fn = createFunction(a, name);
                    if (fn == null) { failed++; continue; }
                    created++;
                }
                try {
                    fn.setName(name, SourceType.IMPORTED);
                    named++;
                } catch (Exception e) {
                    failed++;
                }
            }
        }
        println("methods=" + total + " named=" + named + " created=" + created
            + " noCode=" + noCode + " failed=" + failed);
    }

    private static String clean(String s) {
        return s.replaceAll("[^A-Za-z0-9_]", "_");
    }
}
