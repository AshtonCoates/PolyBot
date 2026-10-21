// Decompiles every function whose name fully matches one of the given regexes.
// Usage: -postScript DecompileFunctions.java <outDir> <regex> [<regex> ...]
// Per-function time limit: environment variable DECOMP_TIMEOUT (seconds, default 120).
// Writes <outDir>/<function name>.c. Works on the lean project (no auto-analysis needed).
//@category IL2CPP
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionIterator;
import java.io.File;
import java.io.FileWriter;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Pattern;

public class DecompileFunctions extends GhidraScript {
    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        File out = new File(args[0]);
        out.mkdirs();
        List<Pattern> pats = new ArrayList<>();
        for (int i = 1; i < args.length; i++) pats.add(Pattern.compile(args[i]));

        String tv = System.getenv("DECOMP_TIMEOUT");
        int timeout = tv == null ? 120 : Integer.parseInt(tv);
        DecompInterface di = new DecompInterface();
        di.openProgram(currentProgram);
        int done = 0, failed = 0;
        FunctionIterator it = currentProgram.getFunctionManager().getFunctions(true);
        while (it.hasNext()) {
            Function f = it.next();
            String name = f.getName();
            boolean hit = false;
            for (Pattern p : pats) if (p.matcher(name).matches()) { hit = true; break; }
            if (!hit) continue;
            DecompileResults r = di.decompileFunction(f, timeout, monitor);
            if (!r.decompileCompleted()) {
                failed++;
                println("FAILED " + name + ": " + r.getErrorMessage());
                continue;
            }
            try (FileWriter w = new FileWriter(new File(out, name + ".c"))) {
                w.write("// " + name + " @ " + f.getEntryPoint() + "\n");
                w.write(r.getDecompiledFunction().getC());
            }
            done++;
        }
        println("decompiled=" + done + " failed=" + failed);
    }
}
