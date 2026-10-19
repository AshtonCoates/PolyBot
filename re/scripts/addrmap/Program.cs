using LibCpp2IL;
using AssetRipper.Primitives;

var dir = args[0];
var bin = Path.Combine(dir, "GameAssembly.so");
var meta = Path.Combine(dir, "Polytopia_Data/il2cpp_data/Metadata/global-metadata.dat");
if (!LibCpp2IlMain.LoadFromFile(bin, meta, UnityVersion.Parse("6000.3.17f1"))) return 1;
using var w = new StreamWriter(args[1]);
w.WriteLine("va\trva\tfileoff\ttype\tmethod\tsig");
foreach (var m in LibCpp2IlMain.TheMetadata!.methodDefs)
{
    if (m.MethodPointer == 0) continue;
    w.WriteLine($"0x{m.MethodPointer:x}\t0x{m.Rva:x}\t0x{m.MethodOffsetInFile:x}\t{m.DeclaringType?.FullName}\t{m.Name}\t{m.HumanReadableSignature}");
}
return 0;
