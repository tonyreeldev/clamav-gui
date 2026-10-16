"""Appends the history of every earlier version (from CHANGELOG.md) to a release's notes.

Every GitHub release keeps the full change history, so people landing on the latest
release see what changed in all previous versions too.

Usage: python scripts/release-notes.py <version> <notes.md> <output.md>
  <notes.md> holds this version's notes and ends with the SmartScreen tip and credits,
  which stay at the bottom.
"""
import re
import sys

CAMBIOS_URL = "https://github.com/tonyreeldev/clamav-gui/blob/main/CAMBIOS.md"
FOOTER_START = "> Windows SmartScreen"


def history(changelog: str, version: str) -> str:
    sections = re.split(r"(?m)^## ", changelog)[1:]
    titles = [s.partition("\n")[0].split(" ")[0] for s in sections]
    if version not in titles:
        sys.exit(f"{version} is not in CHANGELOG.md")
    older = sections[titles.index(version) + 1:]
    blocks = ["## 📜 Historial de versiones", ""]
    for section in older:
        title, _, content = section.partition("\n")
        content = content.strip().replace("[CAMBIOS.md](CAMBIOS.md)", f"[CAMBIOS.md]({CAMBIOS_URL})")
        # "### Añadido" headings read better as bold labels inside a collapsed block.
        content = re.sub(r"(?m)^### (.+)$", r"**\1**", content)
        blocks += [f"<details><summary><b>{title.strip()}</b></summary>", "", content, "", "</details>", ""]
    return "\n".join(blocks)


def main() -> None:
    version, notes_path, out_path = sys.argv[1:4]
    notes = open(notes_path, encoding="utf-8").read().rstrip()
    changelog = open("CHANGELOG.md", encoding="utf-8").read()
    cut = notes.find(FOOTER_START)
    body, footer = (notes, "") if cut < 0 else (notes[:cut].rstrip(), notes[cut:])
    out = f"{body}\n\n{history(changelog, version)}\n{footer}\n"
    open(out_path, "w", encoding="utf-8", newline="\n").write(out)


if __name__ == "__main__":
    main()
