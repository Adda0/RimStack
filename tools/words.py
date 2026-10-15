#!/usr/bin/env python3
"""Wraps the interface's texts in Theme.say() and lists them.

Run after adding or changing a text in the QML or in Rust:  tools/words.py
It rewrites the QML in place (already wrapped texts are left alone) and
writes crates/app/assets/words.json, the list a translation starts from.
"""
import glob, json, os, re

root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "crates", "app")
# `name` and `group` are texts only in the list of commands; elsewhere
# they are keys (an icon's name, a setting's).
shown = r"(?<![\w.])(text|title|tip|placeholder):\s*"
in_commands = r"(?<![\w.])(text|title|tip|placeholder|name|group):\s*"
literal = r'"((?:[^"\\\n]|\\.)*)"'
words = set()
for path in sorted(glob.glob(os.path.join(root, "qml", "**", "*.qml"), recursive=True)):
    if os.sep + "checks" + os.sep in path:
        continue
    source = open(path, encoding="utf-8").read()

    def wrap(found):
        text = found.group(2)
        if not re.search(r"[A-Za-z]{2}", text):
            return found.group(0)
        return f'{found.group(1)}: Theme.say("{text}")'

    named = in_commands if path.endswith("Main.qml") else shown
    # A comment may quote a text; it shows none.
    commented = lambda line: line.lstrip().startswith("//")
    # Theme.qml's own literals are keys and colours, not texts to wrap.
    keep = lambda line: commented(line) or path.endswith("Theme.qml")
    lines = [line if keep(line) else re.sub(named + literal, wrap, line) for line in source.split("\n")]
    if "\n".join(lines) != source:
        open(path, "w", encoding="utf-8").write("\n".join(lines))
    wrapped = "\n".join(line for line in lines if not commented(line))
    # Theme.qml calls its own functions without the name.
    own = r"(?:Theme\.|(?<![.\w]))" if path.endswith("Theme.qml") else r"Theme\."
    words.update(json.loads(f'"{t}"') for t in re.findall(own + r"(?:say|fill)\(" + literal, wrapped))
    # A sentence chosen between two: Theme.fill(shown ? "{} of {}, in {}" : "{} of {}", …).
    for pair in re.findall(r"Theme\.fill\([^\"\n(),]*\?\s*" + literal + r"\s*:\s*" + literal, wrapped):
        words.update(json.loads(f'"{t}"') for t in pair)
    for pair in re.findall(own + r"plural\([^\"\n]*" + literal + r"\s*,\s*" + literal, wrapped):
        words.update(json.loads(f'"{t}"') for t in pair)
    # Lists of texts that are translated where they are shown.
    for line in wrapped.split("\n"):
        if re.search(r"\b(options|words|model|labels):\s*[\[(]", line):
            words.update(json.loads(f'"{t}"') for t in re.findall(literal, line) if re.search(r"[A-Za-z]{2}", t))

# The lists in Theme.qml that are put in the chosen language where shown.
theme = open(os.path.join(root, "qml", "Theme.qml"), encoding="utf-8").read()
for block in re.findall(r"(?:swatches|terms): [\[(]\{?(.*?)\n    \}?[\])]|(?:categoryNames|themeLabels): \[(.*?)\]", theme, re.S):
    words.update(json.loads(f'"{t}"') for part in block for t in re.findall(literal, part) if re.search(r"[A-Za-z]{2}", t) and not t.startswith("#"))
words.update(json.loads(f'"{t}"') for t in re.findall(r"(?<![\w.])(?:say|fill)\(" + literal, theme))
words.update(json.loads(f'"{t}"') for t in re.findall(r"\"@\w+\": " + literal, theme))
# The sentences written in Rust: `say!("…")`, `said!("…")` and `named("…")`.
rust = glob.glob(os.path.join(root, "..", "*", "src", "**", "*.rs"), recursive=True)
# A file declared `#[cfg(test)] mod name;` is a test: beside `main.rs`,
# `lib.rs` or `mod.rs`, otherwise in the folder named after its parent.
tests = set()
for path in rust:
    for name in re.findall(r"#\[cfg\(test\)\]\s*mod (\w+);", open(path, encoding="utf-8").read()):
        stem, folder = os.path.splitext(os.path.basename(path))[0], os.path.dirname(path)
        inside = folder if stem in ("main", "lib", "mod") else os.path.join(folder, stem)
        tests.add(os.path.normpath(os.path.join(inside, name + ".rs")))
for path in rust:
    if os.path.normpath(path) in tests:
        continue
    source = re.split(r"#\[cfg\(test\)\]\s*mod \w+\s*\{", open(path, encoding="utf-8").read())[0]
    words.update(json.loads(f'"{t}"') for t in re.findall(r"(?<!\w)(?:say!|said!|named)\(\s*" + literal, source))
    for pair in re.findall(r"say_n!\([^\"]*" + literal + r"\s*,\s*" + literal, source):
        words.update(json.loads(f'"{t}"') for t in pair)

# A plural or a sentence with places whose texts start on another line
# would not be found above, and would stay English: refused here.
split = []
for path in sorted(glob.glob(os.path.join(root, "qml", "**", "*.qml"), recursive=True)):
    for number, line in enumerate(open(path, encoding="utf-8"), 1):
        if re.search(r"Theme\.(plural|fill)\([^\"]*$", line) and not line.lstrip().startswith("//"):
            split.append(f"{os.path.relpath(path, root)}:{number}")
if split:
    raise SystemExit("texts must start on the line of their call: " + ", ".join(split))

# The language's locale, for its plural rules and to be found from the
# system's settings: "fr", "pt-BR".
words.add("@locale")
out = os.path.join(root, "assets", "words.json")
json.dump({w: "" for w in sorted(words)}, open(out, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
# English is a language like the others: each text beside itself, unless
# its file already words it differently.
english = os.path.join(root, "assets", "languages", "English.json")
kept = json.load(open(english, encoding="utf-8")) if os.path.exists(english) else {}
said = {w: kept.get(w) or ("en" if w == "@locale" else w) for w in sorted(words)}
json.dump(said, open(english, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
print(len(words), "texts")
