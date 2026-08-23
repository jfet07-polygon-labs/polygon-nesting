#!/usr/bin/env python3
"""**The gate text is the spec's, byte for byte, and this is what proves it.**

    python3 section0.py            # check
    python3 section0.py --json     # check, and print the document

Wave 4's first instruction is *"copy the spec's §0 verbatim into
economics-round/README.md §0 BEFORE any gate number"*. Copying it is an act
that happens once; **staying** copied is a property, and a property nobody
checks is one that quietly stops holding the first time a number arrives that
would read better against a slightly different clause.

So this script re-extracts §0 from `docs/economics-round-spec.md` by its own
heading rather than by a line number, un-quotes the block-quoted copy in
`economics-round/README.md`, and requires the two to be **equal as strings**.
Not "equivalent", not "the same numbers": equal. A single digit moved in either
file is a non-zero exit.

The exit status is the verdict, taken directly and never through a pipe:

* `0` - the README's §0 is the spec's §0.
* `1` - they differ. The document names the first line at which they diverge.
* `2` - the check could not run: a file is missing, or the heading it anchors
  on is not in it.

**Why the anchor is the heading and not a line range.** A line range is a
promise about a file that is still being edited; the audit's own repair history
is full of checks that validated the wrong bytes because a constant named a
place rather than a thing. `## §0` is the thing.
"""
import hashlib
import json
import os
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                    '..', '..', '..', '..', '..'))
SPEC = f'{ROOT}/docs/economics-round-spec.md'
README = f'{ROOT}/docs/experiments/overlap-ics/economics-round/README.md'
HEADING = '## §0'


def spec_section0(text):
    """§0 from the spec: its heading, to the next `## ` heading or EOF."""
    lines = text.split('\n')
    start = next((i for i, line in enumerate(lines)
                  if line.startswith(HEADING)), None)
    if start is None:
        raise LookupError(f'{SPEC}: no line starts with {HEADING!r}')
    end = next((i for i in range(start + 1, len(lines))
                if lines[i].startswith('## ')), len(lines))
    # Trailing blank lines belong to the document, not to the section.
    while end > start and not lines[end - 1].strip():
        end -= 1
    return '\n'.join(lines[start:end])


def readme_section0(text):
    """The README's block-quoted copy, un-quoted.

    The copy lives inside a `> ` quote so a reader can see at a glance that it
    is quoted material rather than this directory's own prose. Un-quoting is
    the only transformation applied, and it is exact: `> ` for a line with
    content, `>` alone for a blank one.
    """
    lines = text.split('\n')
    start = next((i for i, line in enumerate(lines)
                  if line.startswith('> ' + HEADING)), None)
    if start is None:
        raise LookupError(f'{README}: no quoted line starts with {HEADING!r}')
    out = []
    for line in lines[start:]:
        if line == '>':
            out.append('')
        elif line.startswith('> '):
            out.append(line[2:])
        else:
            break
    while out and not out[-1].strip():
        out.pop()
    return '\n'.join(out)


def first_difference(left, right):
    left_lines, right_lines = left.split('\n'), right.split('\n')
    for index in range(max(len(left_lines), len(right_lines))):
        a = left_lines[index] if index < len(left_lines) else '<absent>'
        b = right_lines[index] if index < len(right_lines) else '<absent>'
        if a != b:
            return {'line': index + 1, 'spec': a, 'readme': b}
    return None


def main():
    document = {
        'experiment': 'overlap-ics',
        'battery': 'economics-round-section0-verbatim',
        'spec': SPEC,
        'readme': README,
        'anchor': HEADING,
    }
    try:
        with open(SPEC, encoding='utf-8') as handle:
            spec_text = handle.read()
        with open(README, encoding='utf-8') as handle:
            readme_text = handle.read()
        section = spec_section0(spec_text)
        copied = readme_section0(readme_text)
    except (OSError, LookupError) as error:
        document['error'] = f'{error}'
        document['SECTION0_VERBATIM'] = False
        print(json.dumps(document, indent=1, ensure_ascii=False))
        return 2

    document['specSha256'] = hashlib.sha256(spec_text.encode()).hexdigest()
    document['sectionSha256'] = hashlib.sha256(section.encode()).hexdigest()
    document['copySha256'] = hashlib.sha256(copied.encode()).hexdigest()
    document['sectionLines'] = len(section.split('\n'))
    document['sectionBytes'] = len(section.encode())
    document['firstDifference'] = first_difference(section, copied)
    document['SECTION0_VERBATIM'] = section == copied
    document['section'] = section
    if '--json' in sys.argv[1:] or not document['SECTION0_VERBATIM']:
        print(json.dumps(document, indent=1, ensure_ascii=False))
    else:
        print('SECTION0_VERBATIM: true '
              f'({document["sectionLines"]} lines, '
              f'{document["sectionBytes"]} bytes, '
              f'sha256 {document["sectionSha256"][:16]}…)')
    out = os.environ.get('ICS_OUT')
    if out:
        os.makedirs(out, exist_ok=True)
        with open(f'{out}/section0.json', 'w') as handle:
            json.dump(document, handle, indent=1, ensure_ascii=False)
    return 0 if document['SECTION0_VERBATIM'] else 1


if __name__ == '__main__':
    sys.exit(main())
