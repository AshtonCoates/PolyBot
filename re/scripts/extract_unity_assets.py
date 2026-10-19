import UnityPy, sys, os
src, out = sys.argv[1], sys.argv[2]
env = UnityPy.load(src)
from collections import Counter
c = Counter(o.type.name for o in env.objects)
print(c.most_common(15))
n=0
for o in env.objects:
    if o.type.name == "TextAsset":
        d = o.read()
        name = d.m_Name or f"text_{o.path_id}"
        raw = d.m_Script.encode("utf-8","surrogateescape") if isinstance(d.m_Script,str) else bytes(d.m_Script)
        open(os.path.join(out, f"{name}_{o.path_id}.txt"), "wb").write(raw)
        n+=1
        print(name, len(raw))
print("text assets", n)
