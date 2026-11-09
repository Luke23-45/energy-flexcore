import sys

def strip(code):
    out = []
    i = 0
    n = len(code)
    while i < n:
        c = code[i]
        nxt = code[i + 1] if i + 1 < n else ""
        if c == "/" and nxt == "/":
            while i < n and code[i] != "\n":
                i += 1
        elif c == "/" and nxt == "*":
            i += 2
            while i + 1 < n and not (code[i] == "*" and code[i + 1] == "/"):
                i += 1
            i += 2
        elif c == '"':
            i += 1
            while i < n:
                if code[i] == "\\":
                    i += 2
                    continue
                if code[i] == '"':
                    i += 1
                    break
                i += 1
            out.append('""')
        elif c == "'":
            j = i + 1
            if j < n and (code[j].isalpha() or code[j] == "_"):
                while j < n and (code[j].isalnum() or code[j] == "_"):
                    j += 1
                i = j
            else:
                i += 1
                while i < n:
                    if code[i] == "\\":
                        i += 2
                        continue
                    if code[i] == "'":
                        i += 1
                        break
                    i += 1
                out.append("''")
        else:
            out.append(c)
            i += 1
    return "".join(out)

f = sys.argv[1]
code = open(f, encoding="utf-8").read()
t = strip(code)
depth_b = 0
depth_p = 0
for ln, line in enumerate(t.split("\n"), 1):
    d0, p0 = depth_b, depth_p
    depth_b += line.count("{") - line.count("}")
    depth_p += line.count("(") - line.count(")")
    if depth_b < 0 or depth_p < 0:
        print("NEGATIVE at line", ln, "brace", depth_b, "paren", depth_p, "->", line.strip()[:90])
print("final brace", depth_b, "paren", depth_p)
